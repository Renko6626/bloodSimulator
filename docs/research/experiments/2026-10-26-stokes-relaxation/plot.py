#!/usr/bin/env python3
"""Plot archived results without running or altering the simulation."""
import csv
import math
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


def main():
    frozen = read(ROOT / "solver/results/frozen.csv")
    reference = read(ROOT.parent / "2026-10-26-stokes-64-cells/reference-results/optimized-t4-r1/cells.csv")
    fig, axes = plt.subplots(1, 2, figsize=(11, 4.4), layout="constrained")
    colors = {1.0: "#ae4039", 0.5: "#167b88", 0.3: "#bd7a18"}
    for omega in (1.0, 0.5, 0.3):
        rows = [r for r in frozen if r["layout"] == "64" and r["seed"] == "poiseuille"
                and float(r["omega"]) == omega]
        initial = float(rows[0]["velocity_norm"])
        axes[0].semilogy([int(r["iteration"]) for r in rows],
                         [float(r["velocity_norm"]) / initial for r in rows],
                         color=colors[omega], label=f"omega = {omega}")
    axes[0].set(title="Frozen 64-cell feedback", xlabel="Update", ylabel="Velocity norm / initial norm")
    axes[0].axhline(1, color="gray", lw=0.7, ls="--")
    axes[0].legend()
    for omega, case in ((0.5, "omega05-64-r1"), (0.3, "omega03-64-r1")):
        candidate = read(ROOT / "results" / case / "cells.csv")
        assert len(candidate) == len(reference)
        by_frame = {}
        for a, b in zip(candidate, reference):
            assert (a["frame"], a["cell"]) == (b["frame"], b["cell"])
            difference = math.hypot(float(a["center_x"]) - float(b["center_x"]),
                                    float(a["center_y"]) - float(b["center_y"])) / 0.09 * 100
            t = float(a["t"])
            by_frame[t] = max(by_frame.get(t, 0), difference)
        axes[1].plot(list(by_frame), list(by_frame.values()), color=colors[omega], label=f"omega = {omega}")
    axes[1].axvspan(0, 0.5, color="gray", alpha=0.12, label="Drive applied")
    axes[1].set(title="Moving cells vs self-consistent reference", xlabel="Model time (s)",
                ylabel="Max center difference (% of diameter)", ylim=(0, None), xlim=(0, 6))
    axes[1].legend()
    for ax in axes:
        ax.grid(alpha=0.18)
        ax.spines[["top", "right"]].set_visible(False)
    fig.suptitle("64 x 64 grid, 64 permeable cells: relaxation experiment", fontsize=13)
    output = ROOT / "figures"
    output.mkdir(exist_ok=True)
    fig.savefig(output / "relaxation.png", dpi=180)
    fig.savefig(output / "relaxation.svg")
    print(output / "relaxation.png")


if __name__ == "__main__":
    main()
