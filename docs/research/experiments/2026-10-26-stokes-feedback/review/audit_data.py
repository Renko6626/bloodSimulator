#!/usr/bin/env python3
"""Read-only recomputation of the Stokes feedback experiment's CSV summaries."""
import csv
import math
import statistics
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def rows(path):
    with path.open(newline="") as stream:
        return list(csv.DictReader(stream))


static = rows(ROOT / "solver/results/samples.csv")
lagged = rows(ROOT / "solver/results/lagged_samples.csv")
print("static/lagged rows", len(static), len(lagged))
for key in ("force_law_rms", "divergence_rms", "momentum_rms", "grid_force_balance_abs"):
    print("static max", key, max(float(r[key]) for r in static))
for scenario in ("stationary", "moving"):
    for n, cells in ((32, 16), (64, 16), (64, 32), (128, 64)):
        samples = [float(r["total_ms"]) for r in static
                   if (r["scenario"], int(r["n"]), int(r["cells"])) == (scenario, n, cells)]
        print("static timing", scenario, n, cells, len(samples), statistics.mean(samples))
for r in lagged:
    print("static lagged", r["scenario"], r["n"], r["cells"],
          r["total_ms"], r["force_law_rms"])

result_dir = ROOT / "dynamics/results"
for case in sorted(result_dir.iterdir()):
    path = case / "frames.csv"
    if not path.exists():
        continue
    frame_rows = rows(path)
    assert len(frame_rows) == 360
    assert all(int(r["frame"]) == i for i, r in enumerate(frame_rows, 1))
    assert all(abs(float(r["t"]) - i / 60) < 1e-9
               for i, r in enumerate(frame_rows, 1))
    if case.name.endswith("-16"):
        assert abs(sum(float(r["drive_impulse_x"]) for r in frame_rows) - 0.18) < 1e-8
    timed = sorted(float(r["whole_ms"]) for r in frame_rows[30:])
    first_contact = next((int(r["frame"]) for r in frame_rows
                          if int(r["solver_contacts_max"]) > 0), None)
    print("dynamic", case.name, "mean/p95/max",
          statistics.mean(timed), timed[math.ceil(0.95 * len(timed)) - 1], max(timed),
          "first contact", first_contact,
          "max force-law", max(float(r["stokes_force_law_rms"]) for r in frame_rows),
          "max impulse mismatch", max(float(r["exchange_mismatch"]) for r in frame_rows),
          "frame15 vx/gap/contact",
          frame_rows[14]["non_driven_max_abs_vx"],
          frame_rows[14]["driven_nondriven_min_gap"],
          frame_rows[14]["solver_contacts_max"])

for n in (32, 64):
    reference = rows(result_dir / f"stokes{n}-16/cells.csv")
    for backend in ("lagged", "lbm", "control"):
        path = result_dir / f"{backend}{n}-16/cells.csv"
        if not path.exists():
            continue
        other = rows(path)
        assert len(reference) == len(other) == 360 * 16
        assert all((a["frame"], a["cell"]) == (b["frame"], b["cell"])
                   for a, b in zip(reference, other))
        centers = [math.hypot(float(a["center_x"]) - float(b["center_x"]),
                              float(a["center_y"]) - float(b["center_y"]))
                   for a, b in zip(reference, other)]
        velocities = [math.hypot(float(a["velocity_x"]) - float(b["velocity_x"]),
                                 float(a["velocity_y"]) - float(b["velocity_y"]))
                      for a, b in zip(reference, other)]
        print("trajectory", n, backend, "max/mean center",
              max(centers), statistics.mean(centers), "max velocity", max(velocities))
