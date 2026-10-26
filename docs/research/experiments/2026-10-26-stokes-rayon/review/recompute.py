#!/usr/bin/env python3
"""Read-only audit of the Rayon experiment's recorded frame and trajectory CSVs."""
import csv
import hashlib
import json
import math
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "results"
TIMED = {"whole_ms", "fluid_ms", "rapier_ms", "diagnostic_ms"}


def read(path):
    with path.open(newline="") as stream:
        return list(csv.DictReader(stream))


def stats(values):
    ordered = sorted(values)
    return (statistics.mean(ordered), ordered[math.ceil(0.95 * len(ordered)) - 1], ordered[-1])


manifest = json.loads((RESULTS / "manifest.json").read_text())
assert all(hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == digest
           for name, digest in manifest["sources"].items())
pooled = {}
for entry in manifest["runs"]:
    name = entry["name"]
    folder = RESULTS / name
    rows = read(folder / "frames.csv")
    assert entry["exit_code"] == 0 and len(rows) == entry["frames"]
    assert all(int(r["frame"]) == i and abs(float(r["t"]) - i / 60) < 1e-9
               for i, r in enumerate(rows, 1))
    assert all(math.isfinite(float(value)) for row in rows for key, value in row.items()
               if key != "backend")
    baseline = RESULTS / f"n{entry['n']}-t0-r1"
    base_rows = read(baseline / "frames.csv")
    assert len(base_rows) == len(rows)
    assert all({k: v for k, v in a.items() if k not in TIMED} ==
               {k: v for k, v in b.items() if k not in TIMED}
               for a, b in zip(base_rows, rows))
    for filename in ("cells.csv", "outlines.csv", "fields.csv"):
        assert (baseline / filename).read_bytes() == (folder / filename).read_bytes()
    values = [float(r["whole_ms"]) for r in rows[30:]]
    pooled.setdefault((entry["n"], entry["threads"]), []).extend(values)
    print(name, "samples", len(values), "mean/p95/max", *stats(values),
          "init_ms", read(folder / "init.csv")[0]["solver_init_ms"])

for (n, threads), values in sorted(pooled.items()):
    base_mean = stats(pooled[(n, 0)])[0]
    mean, p95, maximum = stats(values)
    print("combined", n, threads, "samples", len(values), "mean/p95/max",
          mean, p95, maximum, "speedup_vs_serial", base_mean / mean)

old = ROOT.parent / "2026-10-26-stokes-feedback/dynamics/results/lagged64-16"
new = RESULTS / "n64-t0-r1"
for filename in ("cells.csv", "outlines.csv", "fields.csv"):
    assert (old / filename).read_bytes() == (new / filename).read_bytes()
print("64^2 serial trajectory matches archived baseline")
