#!/usr/bin/env python3
"""Recompute fluid timings, equation diagnostics and recorded trajectory differences."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parent
DIAMETER = 0.09


def read(path):
    with path.open() as stream:
        return list(csv.DictReader(stream))


def stats(values):
    assert values and all(math.isfinite(v) for v in values)
    ordered = sorted(values)
    return {"mean": statistics.mean(values),
            "p95": ordered[math.ceil(len(ordered) * 0.95) - 1], "max": max(values)}


def compare(left, right):
    assert len(left) == len(right)
    center = velocity = area = 0.0
    for a, b in zip(left, right):
        assert (a["frame"], a["cell"]) == (b["frame"], b["cell"])
        center = max(center, math.hypot(float(a["center_x"]) - float(b["center_x"]),
                                       float(a["center_y"]) - float(b["center_y"])))
        velocity = max(velocity, math.hypot(float(a["velocity_x"]) - float(b["velocity_x"]),
                                           float(a["velocity_y"]) - float(b["velocity_y"])))
        area = max(area, abs(float(a["area_rel_error"]) - float(b["area_rel_error"])))
    return {"max_center_distance": center, "max_center_percent_diameter": center / DIAMETER * 100,
            "max_velocity_distance": velocity, "max_area_error_difference": area,
            "recorded_cell_rows_equal": left == right}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", type=Path, default=ROOT / "results")
    args = parser.parse_args()
    root = args.results
    manifest = json.loads((root / "manifest.json").read_text())
    for path, digest in manifest["sources"].items():
        assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == digest, path
    reference = read(root / "baseline-r1/cells.csv")
    summary = {}
    for run in manifest["runs"]:
        name = run["name"]
        assert run["exit_code"] == 0, name
        frames = read(root / name / "frames.csv")
        assert len(frames) == run["requested_frames"], name
        init = read(root / name / "init.csv")[0]
        assert init["rapier_parallel"] == "true"
        assert int(init["rapier_threads"]) == 4
        assert abs(float(init["prediction"]) - 0.002) < 1e-8
        for row in frames:
            assert all(math.isfinite(float(v)) for v in row.values()), name
            assert float(row["exchange_mismatch"]) <= 1e-6, name
            assert float(row["stokes_divergence_rms"]) <= 1e-8, name
            assert float(row["stokes_momentum_rms"]) <= 1e-8, name
        timed = frames[30:]
        values = {key: stats([float(r[key]) for r in timed]) for key in
                  ("rapier_ms", "fluid_ms", "diagnostic_ms", "whole_ms")}
        values.update({
            "max_area_relative_error": max(float(r["max_area_rel_error"]) for r in frames),
            "max_exchange_mismatch": max(float(r["exchange_mismatch"]) for r in frames),
            "min_driven_nondriven_particle_gap": min(float(r["driven_nondriven_min_gap"]) for r in frames),
            "mean_contacts_per_substep": statistics.mean(float(r["solver_contacts_sum"]) / 2 for r in timed),
            "mean_pressure_iterations": statistics.mean(float(r["stokes_pressure_iterations"]) for r in timed),
            "max_divergence_rms": max(float(r["stokes_divergence_rms"]) for r in frames),
            "max_momentum_rms": max(float(r["stokes_momentum_rms"]) for r in frames),
            "max_force_law_rms": max(float(r["stokes_force_law_rms"]) for r in frames),
            "max_force_law_component": max(float(r["stokes_force_law_max_component_abs"]) for r in frames),
            "max_fluid_calls_per_frame": max(int(r["stokes_fluid_calls"]) for r in frames),
            "trajectory_vs_baseline": compare(reference[:len(frames)*64], read(root / name / "cells.csv")),
            "recorded_outlines_equal": read(root / name / "outlines.csv")
            == [r for r in read(root / "baseline-r1/outlines.csv") if int(r["frame"]) <= len(frames)],
            "recorded_fields_equal": read(root / name / "fields.csv")
            == [r for r in read(root / "baseline-r1/fields.csv") if int(r["frame"]) <= len(frames)],
        })
        if run["profiling"]:
            profile = read(root / name / "profile.csv")[30:]
            values["profile_means"] = {k: statistics.mean(float(row[k]) for row in profile)
                                       for k in profile[0] if k != "frame"}
        summary[name] = values
        print(name, "fluid mean/p95", round(values["fluid_ms"]["mean"], 3),
              round(values["fluid_ms"]["p95"], 3), "whole mean/p95",
              round(values["whole_ms"]["mean"], 3), round(values["whole_ms"]["p95"], 3),
              "center %diam", values["trajectory_vs_baseline"]["max_center_percent_diameter"])
    repeats = {}
    for name in summary:
        if name.endswith("-r1") and name[:-3] + "-r2" in summary:
            repeats[name[:-3]] = compare(read(root / name / "cells.csv"),
                                         read(root / (name[:-3] + "-r2") / "cells.csv"))
    baseline = ROOT.parent / "2026-10-26-rapier-parallel/results/parallel-tuned-t4-r1/cells.csv"
    assert reference == read(baseline), "baseline differs from archived parallel/tuned trajectory"
    (root / "summary.json").write_text(json.dumps({"cases": summary, "repeat_trajectories": repeats,
                                                  "baseline_matches_archive": True}, indent=2) + "\n")


if __name__ == "__main__":
    main()
