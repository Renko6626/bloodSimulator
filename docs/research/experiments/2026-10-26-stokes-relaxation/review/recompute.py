#!/usr/bin/env python3
"""Read-only audit of the relaxation spike's frozen and dynamic CSVs."""
import csv
import hashlib
import json
import math
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "results"
OLD = ROOT.parent / "2026-10-26-stokes-64-cells"


def read(path):
    with path.open(newline="") as stream:
        return list(csv.DictReader(stream))


def stats(values):
    values = sorted(values)
    return statistics.mean(values), values[math.ceil(.95 * len(values)) - 1], values[-1]


manifest = json.loads((RESULTS / "manifest.json").read_text())
assert all(hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == digest
           for name, digest in manifest["sources"].items())
print("source hashes match", len(manifest["sources"]))
for entry in manifest["runs"]:
    name = entry["name"]
    rows = read(RESULTS / name / "frames.csv")
    assert all(int(row["frame"]) == i and abs(float(row["t"]) - i / 60) < 1e-9
               for i, row in enumerate(rows, 1))
    assert all(math.isfinite(float(value)) for row in rows for value in row.values())
    if entry["negative_control"]:
        assert entry["exit_code"] != 0 and len(rows) == 15
        print(name, "expected failed control", len(rows), "frames")
        continue
    assert entry["exit_code"] == 0 and len(rows) == entry["requested_frames"] == 360
    assert max(float(row["exchange_mismatch"]) for row in rows) < 1e-6
    assert int(rows[0]["stokes_fluid_calls"]) == 3
    assert {int(row["stokes_fluid_calls"]) for row in rows[1:]} == {2}
    timed = rows[30:]
    print(name, "whole mean/p95/max", *stats([float(r["whole_ms"]) for r in timed]),
          "point-force mean/p95/max", *stats([float(r["point_force_update_ms"]) for r in timed]),
          "max law rms/component", max(float(r["stokes_force_law_rms"]) for r in rows),
          max(float(r["stokes_force_law_max_component_abs"]) for r in rows),
          "positive frame-power", sum(float(r["exchange_power_mean"]) > 0 for r in rows))
    reference = (OLD / "reference-results/optimized-t4-r1/cells.csv" if entry["count"] == 64 else
                 ROOT.parent / "2026-10-26-stokes-feedback/dynamics/results/stokes64-16/cells.csv")
    cells, ref = read(RESULTS / name / "cells.csv"), read(reference)
    assert len(cells) == len(ref) == 360 * entry["count"]
    assert all((a["frame"], a["cell"]) == (b["frame"], b["cell"])
               for a, b in zip(cells, ref))
    center = [math.hypot(float(a["center_x"]) - float(b["center_x"]),
                         float(a["center_y"]) - float(b["center_y"]))
              for a, b in zip(cells, ref)]
    velocity = [math.hypot(float(a["velocity_x"]) - float(b["velocity_x"]),
                           float(a["velocity_y"]) - float(b["velocity_y"]))
                for a, b in zip(cells, ref)]
    print(name, "max/mean center and max velocity vs self-consistent",
          max(center), statistics.mean(center), max(velocity))

for omega in ("omega05", "omega03"):
    a = RESULTS / f"{omega}-64-r1/cells.csv"
    b = RESULTS / f"{omega}-64-r2/cells.csv"
    assert a.read_bytes() == b.read_bytes()
print("repeat cell trajectories identical")

negative = read(RESULTS / "omega1-64-negative/frames.csv")
archived = read(OLD / "results/optimized-t4-r1/frames.csv")[:15]
timing = {"whole_ms", "fluid_ms", "rapier_ms", "diagnostic_ms"}
common = set(negative[0]) & set(archived[0]) - timing - {"omega", "backend"}
assert all(all(a[key] == b[key] for key in common) for a, b in zip(negative, archived))
print("omega=1 common non-timing frame fields match archived failure", len(common))
new_cells = read(RESULTS / "omega1-64-negative/cells.csv")
old_cells = read(OLD / "results/optimized-t4-r1/cells.csv")[:len(new_cells)]
cell_common = set(new_cells[0]) & set(old_cells[0]) - {"omega", "backend"}
assert len(new_cells) == 15 * 64
assert all(all(a[key] == b[key] for key in cell_common) for a, b in zip(new_cells, old_cells))
print("omega=1 common cell fields match archived failure", len(cell_common))

frozen = read(ROOT / "solver/results/frozen.csv")
for layout, seed in (("16", "poiseuille"), ("64", "poiseuille"), ("64", "local")):
    for omega in ("1.0", "0.5", "0.3"):
        group = [r for r in frozen if (r["layout"], r["seed"], r["omega"]) == (layout, seed, omega)]
        assert len(group) == 33
        ratio = float(group[-1]["velocity_norm"]) / float(group[0]["velocity_norm"])
        middle = statistics.mean(float(r["norm_ratio"]) for r in group if 4 <= int(r["iteration"]) <= 10)
        print("frozen", layout, seed, omega, "final/seed", ratio, "ratio 4-10", middle)
