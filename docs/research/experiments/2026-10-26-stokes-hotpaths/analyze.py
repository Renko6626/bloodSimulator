#!/usr/bin/env python3
"""Check unchanged behavior and separate throughput measurements from profiles."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parent
TIMING = {"whole_ms", "fluid_ms", "rapier_ms", "diagnostic_ms"}


def read(path):
    with path.open(newline="") as stream:
        return list(csv.DictReader(stream))


def describe(values):
    ordered = sorted(values)
    return {"samples": len(values), "mean": statistics.fmean(values),
            "p95": ordered[math.ceil(len(ordered) * 0.95) - 1], "max": max(values)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", type=Path, default=ROOT / "results")
    args = parser.parse_args()
    manifest = json.loads((args.results / "manifest.json").read_text())
    for name, digest in manifest["sources"].items():
        assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == digest, name
    runs, profiles, pooled = {}, {}, {}
    references = {}
    for n in {r["n"] for r in manifest["runs"]}:
        folder = args.results / f"baseline-n{n}-t0-r1"
        references[n] = {f: read(folder / f) for f in ("frames.csv", "cells.csv", "outlines.csv", "fields.csv")}
    for entry in manifest["runs"]:
        name, n = entry["name"], entry["n"]
        assert entry["exit_code"] == 0, name
        folder = args.results / name
        rows = read(folder / "frames.csv")
        assert len(rows) == entry["frames"], name
        for i, row in enumerate(rows, 1):
            assert int(row["frame"]) == i
            assert all(math.isfinite(float(v)) for k, v in row.items() if k != "backend")
            expected = references[n]["frames.csv"][i - 1]
            assert {k: v for k, v in row.items() if k not in TIMING} == {
                k: v for k, v in expected.items() if k not in TIMING}, (name, i, "physics differs")
        for filename in ("cells.csv", "outlines.csv", "fields.csv"):
            current = read(folder / filename)
            frames = {r["frame"] for r in current}
            expected = [r for r in references[n][filename] if r["frame"] in frames]
            assert current == expected, (name, filename)
        timed = rows[30:] if len(rows) > 30 else rows
        item = {"variant": entry["variant"], "n": n, "threads": entry["threads"],
                "repeat": entry["repeat"], "frames": len(rows), "identical_non_timing_outputs": True,
                "solver_init_ms": float(read(folder / "init.csv")[0]["solver_init_ms"]),
                "all_frame_max_ms": max(float(r["whole_ms"]) for r in rows),
                "process_cpu_seconds": entry["process_cpu_seconds"]}
        for column in sorted(TIMING):
            item[column] = describe([float(r[column]) for r in timed])
        if entry["profiling"]:
            segments = read(folder / "profile.csv")
            assert len(segments) == len(rows) * 2
            keep = [r for r in segments if int(r["frame"]) > 30] if len(rows) > 30 else segments
            frame_sums = {}
            for row in keep:
                sums = frame_sums.setdefault(int(row["frame"]), {})
                for key, value in row.items():
                    if key not in {"frame", "substep"}:
                        assert math.isfinite(float(value)) and float(value) >= 0
                        sums[key] = sums.get(key, 0.0) + float(value)
            for frame, sums in frame_sums.items():
                observed = rows[frame - 1]
                expected_calls = int(observed["stokes_pressure_iterations"]) + 2 * int(observed["stokes_fluid_calls"])
                assert sums["inverse_calls"] == expected_calls, (name, frame, "inverse profiling count")
                phase_sum = sum(sums[key] for key in ("inverse_prepare_copy_ms", "forward_fft_ms",
                                "mode_gather_solve_scatter_ms", "inverse_fft_ms"))
                assert phase_sum <= sums["velocity_inverse_ms"] + 1e-6, (name, frame, "nested timing")
            item["profile_per_frame_mean"] = {key: statistics.fmean(r[key] for r in frame_sums.values())
                                                for key in next(iter(frame_sums.values()))}
            profiles[name] = item
        else:
            assert not (folder / "profile.csv").exists(), name
            runs[name] = item
            key = f"{entry['variant']}-n{n}-t{entry['threads']}"
            pooled.setdefault(key, []).extend(float(r["whole_ms"]) for r in timed)
    combined = {key: describe(values) for key, values in pooled.items()}
    for key, item in combined.items():
        _, n, t = key.split("-")
        item["speedup_vs_baseline_same_threads"] = combined[f"baseline-{n}-{t}"]["mean"] / item["mean"]
        item["speedup_vs_baseline_serial"] = combined[f"baseline-{n}-t0"]["mean"] / item["mean"]
    archived_agrees = None
    old = ROOT.parent / "2026-10-26-stokes-rayon/results/n64-t0-r1"
    if len(references[64]["frames.csv"]) == 360:
        archived_agrees = all(read(old / f) == references[64][f] for f in ("cells.csv", "outlines.csv", "fields.csv"))
        assert archived_agrees
    summary = {"runs": runs, "combined": combined, "profiles": profiles,
               "archived_serial_trajectory_identical": archived_agrees,
               "note": "Headline timing excludes profiling. Profile subphases are nested in velocity_inverse_ms; do not sum them with that total."}
    (args.results / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    for key, item in combined.items():
        print(f"{key}: mean/p95/max {item['mean']:.3f}/{item['p95']:.3f}/{item['max']:.3f} ms; "
              f"speedup same threads {item['speedup_vs_baseline_same_threads']:.3f}x")
    for key, item in profiles.items():
        print(key, json.dumps(item["profile_per_frame_mean"]))
    print("All non-timing frame and trajectory/field outputs match baseline serial.")


if __name__ == "__main__":
    main()
