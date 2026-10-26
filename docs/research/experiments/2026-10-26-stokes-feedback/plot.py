#!/usr/bin/env python3
"""Plot the recorded 16-cell experiment; never run or alter the simulation."""
import csv
import json
import math
import os
from pathlib import Path

os.environ.setdefault("MPLCONFIGDIR", "/tmp/blood-stokes-mpl")
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "figures"
COLORS = {"stokes": "#345ea8", "lagged": "#cb7529", "lbm": "#75863d", "control": "#555d66"}
LABELS = {"stokes": "Self-consistent Stokes", "lagged": "One-update Stokes", "lbm": "LBM", "control": "No fluid"}


def read(path):
    with path.open(newline="") as handle:
        return list(csv.DictReader(handle))


def main():
    cases = {}
    for path in sorted((ROOT / "dynamics" / "results").glob("*/frames.csv")):
        rows = read(path)
        if len(rows) != 360 or int(rows[0]["count"]) != 16:
            continue
        key = rows[0]["backend"], int(rows[0]["n"])
        if key in cases:
            raise ValueError(f"duplicate final case {key}")
        cases[key] = (path.parent, rows)
    required = [(backend, n) for n in (32, 64) for backend in ("stokes", "lagged", "lbm")]
    assert all(key in cases for key in required), "Six matched 360-frame cases are required"
    control = next((key for key in cases if key[0] == "control"), None)
    assert control is not None
    OUT.mkdir(exist_ok=True)
    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10,
                         "axes.spines.top": False, "axes.spines.right": False,
                         "axes.titleweight": "bold", "axes.labelcolor": "#323943",
                         "text.color": "#232b35", "svg.fonttype": "none"})
    fig, axes = plt.subplots(2, 2, figsize=(13, 10.4), layout="constrained")
    fig.suptitle("Finite-drag fluid feedback: 16 deformable cells", fontsize=17)
    ax = axes[0, 0]
    derived = []
    for pos, key in enumerate(required):
        folder, rows = cases[key]
        backend, n = key
        samples = sorted(float(r["whole_ms"]) for r in rows[30:])
        mean = float(np.mean(samples))
        p95 = samples[math.ceil(0.95 * len(samples)) - 1]
        ax.bar(pos, mean, width=0.65, color=COLORS[backend])
        ax.plot(pos, p95, marker="D", color="#232b35", markersize=4)
        ax.text(pos, max(mean, p95) + 0.7, f"mean {mean:.2f}", ha="center", fontsize=8)
        derived.append({"backend": backend, "n": n, "mean_ms": mean,
                        "p95_ms": p95, "samples": len(samples),
                        "source": str((folder / "frames.csv").relative_to(ROOT))})
    ax.axhline(5, color="#555d66", linewidth=1, linestyle="--", label="5 ms target")
    ax.set_xticks(range(len(required)), [f"{n} x {n}\n{b}" for b, n in required])
    ax.set_ylim(bottom=0)
    ax.set_ylabel("Whole frame (ms)")
    ax.set_title("A. Frame cost: bars = mean; diamonds = p95", loc="left", fontsize=11)
    ax.grid(axis="y", alpha=0.16)
    ax.set_axisbelow(True)
    ax.legend(frameon=False, fontsize=9)

    ax = axes[0, 1]
    for key in [("stokes", 64), ("lagged", 64), ("lbm", 64), control]:
        _, rows = cases[key]
        backend, _ = key
        rows = [r for r in rows if float(r["t"]) <= 1.4]
        assert all(int(r["solver_contacts_sum"]) == 0 for r in rows)
        ax.plot([float(r["t"]) for r in rows],
                [float(r["non_driven_mean_vx"]) for r in rows],
                color=COLORS[backend], label=LABELS[backend],
                linestyle="--" if backend in ("lagged", "control") else "-", linewidth=1.8)
    ax.axvspan(0, 0.5, color="#8c949c", alpha=0.12)
    ax.set(xlabel="Model time (s)", ylabel="Mean x velocity of 14 unforced cells", xlim=(0, 1.4))
    ax.set_title("B. Unforced cells before contact (64 x 64)", loc="left", fontsize=11)
    ax.ticklabel_format(axis="y", style="sci", scilimits=(0, 0))
    ax.grid(alpha=0.16)
    ax.legend(frameon=False, fontsize=9)

    snapshot_frame = 30
    field_data = []
    for backend in ("stokes", "lagged"):
        folder, _ = cases[(backend, 64)]
        field = [r for r in read(folder / "fields.csv") if int(r["frame"]) == snapshot_frame]
        outlines = [r for r in read(folder / "outlines.csv") if int(r["frame"]) == snapshot_frame]
        x = np.array(sorted({float(r["x"]) for r in field}))
        y = np.array(sorted({float(r["y"]) for r in field}))
        rows = sorted(field, key=lambda r: (float(r["y"]), float(r["x"])))
        u = np.array([float(r["ux"]) for r in rows]).reshape(len(y), len(x))
        v = np.array([float(r["uy"]) for r in rows]).reshape(len(y), len(x))
        field_data.append((backend, x, y, u, v, outlines))
    vmax = max(float(np.hypot(a[3], a[4]).max()) for a in field_data)
    assert vmax > 0
    for ax, entry, panel in zip(axes[1], field_data, ("C", "D")):
        backend, x, y, u, v, outlines = entry
        mesh = ax.pcolormesh(x, y, np.hypot(u, v), shading="nearest", cmap="Blues", vmin=0, vmax=vmax)
        ax.quiver(x, y, u, v, color="#34414d", angles="xy", scale_units="xy", scale=vmax * 19, width=0.003)
        for cell in sorted({int(r["cell"]) for r in outlines}):
            nodes = sorted((r for r in outlines if int(r["cell"]) == cell), key=lambda r: int(r["node"]))
            xy = np.array([[float(r["x"]), float(r["y"])] for r in nodes])
            xy = np.vstack((xy, xy[0]))
            driven = nodes[0]["driven"] == "1"
            ax.plot(xy[:, 0], xy[:, 1], color="#cb7529" if driven else "#555d66",
                    linewidth=1.8 if driven else 0.85)
        ax.axhline(0, color="#232b35", linewidth=2)
        ax.axhline(1, color="#232b35", linewidth=2)
        ax.set(xlim=(0, 1), ylim=(0, 1), aspect="equal", xlabel="x (periodic)", ylabel="y")
        ax.set_title(f"{panel}. {LABELS[backend]}, t = 0.5 s", loc="left", fontsize=11)
    fig.colorbar(mesh, ax=axes[1, :], label="Interpolated fluid speed (model units)", shrink=0.8)
    fig.supxlabel("Source: dynamics/results CSVs. 60 Hz, two mechanical substeps; timings exclude output and rendering.\n"
                  "Cost: frames 31-360. Grey time band: forcing applied to two orange cells. Both fields share scales.\n"
                  "Stokes and LBM have different transient physics; the cells remain permeable. No irregular fluid walls tested.", fontsize=9)
    fig.savefig(OUT / "overview.png", dpi=160)
    fig.savefig(OUT / "overview.svg")
    (OUT / "plot-data.json").write_text(json.dumps({"cost": derived, "field_frame": snapshot_frame,
                                                  "field_speed_max": vmax}, indent=2) + "\n")
    print(OUT / "overview.png")


if __name__ == "__main__":
    main()
