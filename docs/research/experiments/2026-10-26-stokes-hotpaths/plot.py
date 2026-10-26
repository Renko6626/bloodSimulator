#!/usr/bin/env python3
"""Plot measured whole-frame cost and explicitly separate diagnostic profiles."""
import json
import os
from pathlib import Path

os.environ.setdefault("MPLCONFIGDIR", "/tmp/blood-stokes-mpl")
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

ROOT = Path(__file__).resolve().parent
data = json.loads((ROOT / "results/summary.json").read_text())
plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10,
                     "axes.spines.top": False, "axes.spines.right": False,
                     "svg.fonttype": "none"})
fig, axes = plt.subplots(2, 2, figsize=(13, 9.7), layout="constrained")
colors = {"baseline": "#a7b4c3", "optimized": "#38689c"}
for ax, n in zip(axes[0], (64, 128)):
    ceiling = 5.0
    for variant, offset in (("baseline", -0.18), ("optimized", 0.18)):
        for i, threads in enumerate((0, 1, 2, 4)):
            item = data["combined"][f"{variant}-n{n}-t{threads}"]
            ax.bar(i + offset, item["mean"], width=0.34, color=colors[variant],
                   label=variant if i == 0 else None)
            ax.plot(i + offset, item["p95"], "D", color=colors[variant], markersize=4)
            ceiling = max(ceiling, item["p95"], item["mean"])
    ax.axhline(5, color="#59636e", linestyle="--", linewidth=1)
    ax.set_xticks(range(4), ["Serial", "Rayon 1", "Rayon 2", "Rayon 4"])
    ax.set_ylim(0, ceiling * 1.14)
    ax.set_ylabel("Whole frame (ms), profiling disabled")
    ax.set_title(f"{n} x {n}, 16 cells", loc="left", weight="bold")
    ax.legend(frameon=False)
    ax.grid(axis="y", alpha=0.15)
    ax.set_axisbelow(True)

ax = axes[1, 0]
profile_keys = [f"profile-{v}-n64-t{t}-r1" for t in (0, 4) for v in ("baseline", "optimized")]
phase_names = ["prepare/copy", "forward FFT", "mode solve/transpose", "inverse FFT", "inverse remainder"]
phase_colors = ["#c4ccd4", "#3f73a2", "#d6a24b", "#7e9e69", "#a27898"]
segments = []
for key in profile_keys:
    p = data["profiles"][key]["profile_per_frame_mean"]
    row = [p[k] for k in ("inverse_prepare_copy_ms", "forward_fft_ms",
                          "mode_gather_solve_scatter_ms", "inverse_fft_ms")]
    remainder = p["velocity_inverse_ms"] - sum(row)
    assert remainder >= -1e-6, key
    segments.append(row + [max(0.0, remainder)])
bottom = np.zeros(4)
for i, (label, color) in enumerate(zip(phase_names, phase_colors)):
    values = np.array([r[i] for r in segments])
    ax.bar(range(4), values, bottom=bottom, color=color, label=label, width=0.65)
    bottom += values
ax.set_xticks(range(4), ["Base\nserial", "Opt\nserial", "Base\n4 threads", "Opt\n4 threads"])
ax.set_ylabel("Velocity inverses per frame (ms)")
ax.set_title("Diagnostic profile only: 64 x 64", loc="left", weight="bold")
ax.legend(frameon=False, fontsize=8)
ax.grid(axis="y", alpha=0.15)
ax.set_axisbelow(True)

ax = axes[1, 1]
for variant in ("baseline", "optimized"):
    runs = [r for r in data["runs"].values() if r["variant"] == variant and r["n"] == 64]
    for repeat, marker in ((1, "o"), (2, "s")):
        values = [next(r["whole_ms"]["mean"] for r in runs if r["threads"] == t and r["repeat"] == repeat)
                  for t in (0, 1, 2, 4)]
        ax.plot(range(4), values, marker=marker, color=colors[variant],
                linestyle="-" if repeat == 1 else "--", label=f"{variant}, round {repeat}")
ax.axhline(5, color="#59636e", linestyle="--", linewidth=1)
ax.set_xticks(range(4), ["Serial", "Rayon 1", "Rayon 2", "Rayon 4"])
ax.set_ylim(bottom=0)
ax.set_ylabel("Whole-frame mean (ms)")
ax.set_title("64 x 64: both measured rounds", loc="left", weight="bold")
ax.legend(frameon=False, fontsize=8)
ax.grid(alpha=0.15)
fig.suptitle("Stokes hot-path experiment", fontsize=17)
fig.supxlabel("Top: bars = pooled mean, diamonds = pooled p95; independent y scales. Headline runs have profiling disabled.\n"
              "Profile: separate 120-frame runs, last 90 frames shown; subphases sum to velocity-inverse total only.\n"
              "Pool entry is inside the baseline inverse timer and outside the optimized inverse timer; whole-frame timing includes both.\n"
              "Same-socket physical cores, non-exclusive host. Initialization, output and rendering excluded from frame timing.", fontsize=9)
fig.savefig(ROOT / "results/overview.png", dpi=160)
fig.savefig(ROOT / "results/overview.svg")
print(ROOT / "results/overview.png")
