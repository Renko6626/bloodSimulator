#!/usr/bin/env python3
"""Check trajectory equivalence and report every recorded timing sample."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parent
TIMINGS = {"whole_ms", "fluid_ms", "rapier_ms", "diagnostic_ms"}


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
    runs, aggregate = {}, {}
    for entry in manifest["runs"]:
        name, n, threads = entry["name"], entry["n"], entry["threads"]
        assert entry["exit_code"] == 0, name
        folder = args.results / name
        rows = read(folder / "frames.csv")
        assert len(rows) == entry["frames"], name
        for i, row in enumerate(rows, 1):
            assert int(row["frame"]) == i
            assert all(math.isfinite(float(v)) for k, v in row.items() if k != "backend")
        baseline_folder = args.results / f"n{n}-t0-r1"
        baseline = read(baseline_folder / "frames.csv")
        assert len(baseline) == len(rows)
        for a, b in zip(baseline, rows):
            assert {k: v for k, v in a.items() if k not in TIMINGS} == {
                k: v for k, v in b.items() if k not in TIMINGS}, (name, b["frame"], "physics differs")
        for filename in ("cells.csv", "outlines.csv", "fields.csv"):
            assert (baseline_folder / filename).read_bytes() == (folder / filename).read_bytes(), (name, filename)
        timed = rows[30:] if len(rows) > 30 else rows
        init = read(folder / "init.csv")[0]
        result = {"n": n, "threads": threads, "repeat": entry["repeat"], "frames": len(rows),
                  "solver_init_ms": float(init["solver_init_ms"]),
                  "all_frame_max_ms": max(float(r["whole_ms"]) for r in rows),
                  "non_timing_outputs_identical": True,
                  "process_cpu_seconds": entry["process_cpu_seconds"],
                  "process_wall_seconds": entry["process_wall_seconds"]}
        for column in sorted(TIMINGS):
            result[column] = describe([float(r[column]) for r in timed])
        runs[name] = result
        aggregate.setdefault((n, threads), []).extend(float(r["whole_ms"]) for r in timed)
    combined = {f"n{n}-t{t}": describe(values) for (n, t), values in aggregate.items()}
    for (n, t) in aggregate:
        combined[f"n{n}-t{t}"]["speedup_vs_original_serial"] = combined[f"n{n}-t0"]["mean"] / combined[f"n{n}-t{t}"]["mean"]
    old = ROOT.parent / "2026-10-26-stokes-feedback/dynamics/results/lagged64-16"
    current = args.results / "n64-t0-r1"
    old_agrees = None
    if old.exists() and len(read(current / "frames.csv")) == 360:
        old_agrees = all((old / f).read_bytes() == (current / f).read_bytes()
                         for f in ("cells.csv", "outlines.csv", "fields.csv"))
        assert old_agrees, "New serial trajectory changed from archived 64² baseline"
    summary = {"runs": runs, "combined": combined, "old_serial_trajectory_identical": old_agrees,
               "note": "Combined p95 uses pooled frame samples, not the mean of per-run p95s."}
    (args.results / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    for name, item in runs.items():
        t = item["whole_ms"]
        print(f"{name}: mean/p95/max {t['mean']:.3f}/{t['p95']:.3f}/{t['max']:.3f} ms; init {item['solver_init_ms']:.3f} ms")
    print("Combined:")
    for name, t in combined.items():
        print(f"{name}: mean/p95 {t['mean']:.3f}/{t['p95']:.3f} ms, speedup {t['speedup_vs_original_serial']:.3f}x")
    print("All non-timing frame fields and all cell/outline/field files match original serial output.")


if __name__ == "__main__":
    main()
