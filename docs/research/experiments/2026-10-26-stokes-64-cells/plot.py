#!/usr/bin/env python3
import csv
import os
from pathlib import Path

os.environ.setdefault("MPLCONFIGDIR", "/tmp/blood-stokes-mpl")
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parent


def read(path):
    with path.open(newline="") as stream:
        return list(csv.DictReader(stream))


fig, axes = plt.subplots(1, 2, figsize=(12, 5.1), layout="constrained")
plt.rcParams.update({"svg.fonttype": "none"})
for mode, directory, color in (("One-update Stokes (failed)", "results", "#b66b2c"),
                                ("Self-consistent Stokes", "reference-results", "#38689c")):
    rows = [r for r in read(ROOT / directory / "optimized-t4-r1/frames.csv") if float(r["t"]) <= 0.3]
    axes[0].semilogy([float(r["t"]) for r in rows], [float(r["max_fluid_speed"]) for r in rows],
                     label=mode, color=color, marker="o", markersize=3)
axes[0].axvline(16 / 60, color="#555d66", linestyle="--", linewidth=1, label="Check stops frame 16")
axes[0].set(xlabel="Model time (s)", ylabel="Maximum fluid speed (model units)",
            title="64 cells: rapid growth before failure", xlim=(0, 0.3))
for count, color in ((16, "#38689c"), (64, "#b66b2c")):
    rows = read(ROOT / "frozen-results" / f"cells-{count}.csv")
    initial = float(rows[0]["velocity_norm"])
    axes[1].semilogy([int(r["iteration"]) for r in rows],
                     [float(r["velocity_norm"]) / initial for r in rows],
                     label=f"{count} cells, late ratio {float(rows[-1]['norm_ratio']):.3f}",
                     color=color, marker="o", markersize=3)
axes[1].set(xlabel="Number of lagged updates", ylabel="Velocity norm / initial norm",
            title="Frozen geometry: no Rapier or contact updates")
for ax in axes:
    ax.grid(alpha=0.18)
    ax.legend(frameon=False, fontsize=9)
    ax.spines[["top", "right"]].set_visible(False)
fig.suptitle("64-cell density check on a 64 x 64 fluid grid", fontsize=15)
fig.supxlabel("Same cell radius 0.045 and drag 0.25; fixed unit domain. 64 cells form an 8 x 8 layout.\n"
              "Frozen check: all marker velocities zero, seeded flow with its drive removed. Values are raw solver outputs.", fontsize=9)
fig.savefig(ROOT / "failure-and-feedback.png", dpi=160)
fig.savefig(ROOT / "failure-and-feedback.svg")
print(ROOT / "failure-and-feedback.png")
