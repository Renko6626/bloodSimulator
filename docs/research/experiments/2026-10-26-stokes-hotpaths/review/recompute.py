#!/usr/bin/env python3
"""Read-only recomputation of matched hot-path runs, excluding profile samples."""
import csv
import hashlib
import json
import math
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "results"
TIMING = {"whole_ms", "fluid_ms", "rapier_ms", "diagnostic_ms"}


def read(path):
    with path.open(newline="") as stream:
        return list(csv.DictReader(stream))


def summary(values):
    ordered = sorted(values)
    return statistics.mean(ordered), ordered[math.ceil(0.95 * len(ordered)) - 1], ordered[-1]


manifest = json.loads((RESULTS / "manifest.json").read_text())
assert all(hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == digest
           for name, digest in manifest["sources"].items())
static = read(ROOT / "solver/results/compare_baseline.csv")
assert len(static) == 28 and all(row["arrays_and_residuals_bitwise_equal"] == "true"
                                 and float(row["max_abs_delta"]) == 0.0 for row in static)
print("static baseline/optimized checks", len(static), "all bitwise equal")
pooled = {}
headline = [entry for entry in manifest["runs"] if not entry["profiling"]]
for entry in headline:
    name, n, threads, repeat = (entry[key] for key in ("name", "n", "threads", "repeat"))
    folder = RESULTS / name
    rows = read(folder / "frames.csv")
    assert entry["exit_code"] == 0 and len(rows) == entry["frames"]
    assert not (folder / "profile.csv").exists()
    assert all(int(row["frame"]) == i and abs(float(row["t"]) - i / 60) < 1e-9
               for i, row in enumerate(rows, 1))
    assert all(math.isfinite(float(value)) for row in rows for key, value in row.items()
               if key != "backend")
    base = RESULTS / f"baseline-n{n}-t0-r1"
    baseline_rows = read(base / "frames.csv")
    assert len(rows) == len(baseline_rows)
    assert all({k: v for k, v in row.items() if k not in TIMING} ==
               {k: v for k, v in ref.items() if k not in TIMING}
               for row, ref in zip(rows, baseline_rows))
    for filename in ("cells.csv", "outlines.csv", "fields.csv"):
        assert (folder / filename).read_bytes() == (base / filename).read_bytes()
    timed = [float(row["whole_ms"]) for row in rows[30:]]
    pooled.setdefault((entry["variant"], n, threads), []).extend(timed)
    print(name, "samples", len(timed), "mean/p95/max", *summary(timed),
          "init_ms", read(folder / "init.csv")[0]["solver_init_ms"])

for n in (64, 128):
    for threads in (0, 1, 2, 4):
        base = summary(pooled[("baseline", n, threads)])
        opt = summary(pooled[("optimized", n, threads)])
        print("combined", n, threads, "baseline", *base, "optimized", *opt,
              "same_threads_speedup", base[0] / opt[0])

for entry in manifest["runs"]:
    if entry["profiling"]:
        rows = read(RESULTS / entry["name"] / "profile.csv")
        assert len(rows) == 2 * entry["frames"]
        kept = [row for row in rows if int(row["frame"]) > 30]
        means = {key: statistics.mean(
            sum(float(row[key]) for row in kept if int(row["frame"]) == frame)
            for frame in range(31, entry["frames"] + 1))
            for key in ("inverse_calls", "velocity_inverse_ms",
                        "mode_gather_solve_scatter_ms", "inverse_prepare_copy_ms",
                        "pressure_stencil_ms")}
        print("profile", entry["name"], "timed frames", entry["frames"] - 30, means)

old = ROOT.parent / "2026-10-26-stokes-rayon/results/n64-t0-r1"
new = RESULTS / "baseline-n64-t0-r1"
for filename in ("cells.csv", "outlines.csv", "fields.csv"):
    assert (old / filename).read_bytes() == (new / filename).read_bytes()
print("64^2 baseline serial trajectory matches prior Rayon experiment")
