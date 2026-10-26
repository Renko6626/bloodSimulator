#!/usr/bin/env python3
"""Render only recorded timing summaries; no synthetic performance estimates."""
import json
import os
from pathlib import Path

os.environ.setdefault("MPLCONFIGDIR", "/tmp/blood-stokes-mpl")
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parent
summary = json.loads((ROOT / "results/summary.json").read_text())
plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 11,
                     "axes.spines.top": False, "axes.spines.right": False,
                     "svg.fonttype": "none"})
fig, axes = plt.subplots(1, 2, figsize=(12, 5.7), layout="constrained")
colors = ["#697784", "#b8c7d7", "#4678aa", "#244b75"]
for ax, n in zip(axes, (64, 128)):
    for x, threads in enumerate((0, 1, 2, 4)):
        data = summary["combined"][f"n{n}-t{threads}"]
        ax.bar(x, data["mean"], color=colors[x], width=0.65)
        ax.plot(x, data["p95"], "D", color="#bd6d27", markersize=6)
        for repeat, offset in ((1, -0.15), (2, 0.15)):
            value = summary["runs"][f"n{n}-t{threads}-r{repeat}"]["whole_ms"]["mean"]
            ax.plot(x + offset, value, "_", color="#202c39", markersize=9)
        ax.text(x, max(data["p95"], data["mean"]) + 0.6,
                f"mean {data['mean']:.2f}", ha="center", fontsize=9)
    ax.axhline(5, color="#65707d", linestyle="--", linewidth=1)
    ax.set_xticks(range(4), ["Original\nserial", "Rayon\n1 thread", "Rayon\n2 threads", "Rayon\n4 threads"])
    ymax = max(summary["combined"][f"n{n}-t{t}"]["p95"] for t in (0, 1, 2, 4))
    ax.set_ylim(0, ymax * 1.18 + 1)
    ax.set_title(f"{n} x {n}, 16 cells", loc="left", weight="bold")
    ax.set_ylabel("Whole simulation frame (ms)")
    ax.grid(axis="y", alpha=0.15)
    ax.set_axisbelow(True)
fig.suptitle("Rayon experiment: one-update Stokes", fontsize=16)
fig.supxlabel("Bars: pooled means; orange diamonds: pooled p95; black ticks: each round's mean. Dashed line: 5 ms.\n"
              "Same-socket physical cores; two reversed-order rounds; first 30 frames excluded per run.\n"
              "64 x 64: 360 frames/run; 128 x 128: 120 frames/run. Panels use different y scales.\n"
              "Non-exclusive host; initialization/output/rendering excluded.", fontsize=9)
fig.savefig(ROOT / "results/timing.png", dpi=160)
fig.savefig(ROOT / "results/timing.svg")
print(ROOT / "results/timing.png")
