"""Recheck raw MAC/Stokes samples and render the standalone research figure.

Run after `python run.py --runs 50 --cpu 2` has finished.
"""

from __future__ import annotations

import csv
import json
import math
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import NullLocator
import numpy as np


ROOT = Path(__file__).resolve().parent
RESULTS = ROOT / "results"
SIZES = (32, 64, 128)
SCENES = ("poiseuille", "manufactured", "localized")
BACKENDS = ("cg", "separable")


def read_csv(path: Path) -> list[dict[str, str]]:
    with path.open(newline="") as stream:
        return list(csv.DictReader(stream))


def p95(values: list[float]) -> float:
    values = sorted(values)
    return values[math.ceil((len(values) - 1) * 0.95)]


def close(actual: float, reported: float, field: str, key: str) -> None:
    # Summary timings are printed to 9 decimal places.
    if not math.isclose(actual, reported, rel_tol=2e-8, abs_tol=2e-6):
        raise ValueError(f"{key}: {field} recomputed={actual} summary={reported}")


def load_and_check() -> tuple[dict, dict]:
    summary_rows = read_csv(RESULTS / "summary.csv")
    if len(summary_rows) != len(BACKENDS) * len(SCENES) * len(SIZES):
        raise ValueError("summary.csv is incomplete; wait for run.py")
    summary = {(r["backend"], r["scene"], int(r["n"])): r for r in summary_rows}
    if len(summary) != len(summary_rows):
        raise ValueError("duplicate summary keys")
    checks: dict[str, dict] = {}
    for backend in BACKENDS:
        samples = read_csv(RESULTS / f"{backend}-samples.csv")
        for scene in SCENES:
            for n in SIZES:
                key = (backend, scene, n)
                if key not in summary:
                    raise ValueError(f"missing {key}")
                row = summary[key]
                subset = [s for s in samples if (s["backend"], s["scene"], int(s["n"])) == key]
                expected = {"cold": 50, "warm": 49 if scene == "manufactured" else 0}
                entry = {"counts": {}, "timing_ms": {}}
                for state, count in expected.items():
                    selected = [s for s in subset if s["cold_or_warm"] == state]
                    if len(selected) != count:
                        raise ValueError(f"{key} {state}: expected {count}, got {len(selected)}")
                    if sorted(int(s["rep"]) for s in selected) != list(range(count)):
                        # Warm samples start at repetition 1, unlike cold samples.
                        if not (state == "warm" and sorted(int(s["rep"]) for s in selected) == list(range(1, count + 1))):
                            raise ValueError(f"{key} {state}: unexpected repetition indices")
                    entry["counts"][state] = len(selected)
                    if selected:
                        values = [float(s["elapsed_ms"]) for s in selected]
                        mean = sum(values) / len(values)
                        q95 = p95(values)
                        close(mean, float(row[f"{state}_ms_mean"]), f"{state}_ms_mean", str(key))
                        close(q95, float(row[f"{state}_ms_p95"]), f"{state}_ms_p95", str(key))
                        entry["timing_ms"][state] = {"mean": mean, "p95": q95, "min": min(values), "max": max(values)}
                checks[f"{backend}/{scene}/{n}"] = entry
        if len(samples) != 9 * 50 + 3 * 49:
            raise ValueError(f"unexpected total {backend} sample count: {len(samples)}")

    accuracy = {}
    for backend in BACKENDS:
        vel = [float(summary[(backend, "manufactured", n)]["u_l2"]) for n in SIZES]
        pressure = [float(summary[(backend, "manufactured", n)]["p_l2"]) for n in SIZES]
        accuracy[backend] = {
            "velocity_l2": dict(zip(map(str, SIZES), vel)),
            "pressure_l2": dict(zip(map(str, SIZES), pressure)),
            "velocity_refinement_ratios": [vel[0] / vel[1], vel[1] / vel[2]],
            "pressure_refinement_ratios": [pressure[0] / pressure[1], pressure[1] / pressure[2]],
            "velocity_orders": [math.log2(vel[0] / vel[1]), math.log2(vel[1] / vel[2])],
            "pressure_orders": [math.log2(pressure[0] / pressure[1]), math.log2(pressure[1] / pressure[2])],
        }

    residuals = {}
    for backend in BACKENDS:
        rows = [summary[(backend, scene, n)] for scene in SCENES for n in SIZES]
        residuals[backend] = {
            "cold_divergence_rms_range": [min(float(r["div_rms"]) for r in rows), max(float(r["div_rms"]) for r in rows)],
            "cold_momentum_rms_range": [min(float(r["momentum_rms"]) for r in rows), max(float(r["momentum_rms"]) for r in rows)],
            "manufactured_warm_divergence_rms_max": max(float(summary[(backend, "manufactured", n)]["warm_div_rms_max"]) for n in SIZES),
            "manufactured_warm_momentum_rms_max": max(float(summary[(backend, "manufactured", n)]["warm_momentum_rms_max"]) for n in SIZES),
        }

    analysis = {
        "scope": "periodic-x/no-slip-y regular channel; no cell, irregular wall, wall IB, Rapier, or game step",
        "sources": ["results/summary.csv", "results/cg-samples.csv", "results/separable-samples.csv", "results/fields/localized_64_cells.csv"],
        "aggregation": "arithmetic mean; p95 = sorted[ceil((N-1)*0.95)]",
        "sample_checks": checks,
        "manufactured_accuracy": accuracy,
        "true_residual_rms": residuals,
        "localized_cold_ms": {
            str(n): {backend: checks[f"{backend}/localized/{n}"]["timing_ms"]["cold"] for backend in BACKENDS}
            for n in SIZES
        },
        "timing_excludes": "backend initialization, grid/RHS initialization, export, cells, wall IB, Rapier, rendering",
    }
    return summary, analysis


def load_field() -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    path = RESULTS / "fields" / "localized_64_cells.csv"
    rows = read_csv(path)
    n = 64
    if len(rows) != n * n:
        raise ValueError(f"expected {n*n} numerical field rows in {path}")
    x = np.array([float(r["x"]) for r in rows]).reshape(n, n)
    y = np.array([float(r["y"]) for r in rows]).reshape(n, n)
    u = np.array([float(r["ux_center"]) for r in rows]).reshape(n, n)
    v = np.array([float(r["uy_center"]) for r in rows]).reshape(n, n)
    p = np.array([float(r["p"]) for r in rows]).reshape(n, n)
    if not (np.all(np.diff(x[0]) > 0) and np.all(np.diff(y[:, 0]) > 0)):
        raise ValueError("field grid is not ordered as expected")
    return x, y, u, v, p


def render(summary: dict, analysis: dict) -> None:
    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10, "axes.titlesize": 12,
                         "axes.labelsize": 10, "svg.fonttype": "none"})
    fig = plt.figure(figsize=(12.5, 10.3), constrained_layout=True)
    grid = fig.add_gridspec(3, 2, height_ratios=(1, 1, 0.07))
    axes = np.array([[fig.add_subplot(grid[0, 0]), fig.add_subplot(grid[0, 1])],
                     [fig.add_subplot(grid[1, 0]), fig.add_subplot(grid[1, 1])]])
    note_ax = fig.add_subplot(grid[2, :])
    note_ax.axis("off")
    fig.suptitle("Steady MAC–Stokes: solver cost and numerical checks", fontsize=16, fontweight="bold")
    blue, orange, ink = "#245A81", "#C66A2C", "#273544"

    ax = axes[0, 0]
    for backend, color, label in (("cg", blue, "Nested CG"), ("separable", orange, "FFT + tridiagonal")):
        keys = [f"{backend}/localized/{n}" for n in SIZES]
        mean = [analysis["sample_checks"][key]["timing_ms"]["cold"]["mean"] for key in keys]
        q95 = [analysis["sample_checks"][key]["timing_ms"]["cold"]["p95"] for key in keys]
        ax.plot(SIZES, mean, "o-", color=color, lw=2, label=f"{label} mean")
        ax.plot(SIZES, q95, "s--", color=color, lw=1.5, mfc="white", label=f"{label} p95")
    ax.axhline(5, color=ink, lw=1, ls=":")
    ax.text(33, 5.6, "5 ms whole game-step aspiration", color=ink, fontsize=8)
    ax.set(xlabel="Grid n × n", ylabel="Complete cold Stokes solve (ms)", title="Localized forcing: complete solve time")
    ax.set_xticks(SIZES)
    ax.set_yscale("log")
    ax.grid(axis="y", color="#DCE2E6", alpha=0.8)
    ax.legend(frameon=False, fontsize=8, loc="upper left")

    ax = axes[0, 1]
    for metric, color, marker, label in (("u_l2", blue, "o", "Velocity RMS"), ("p_l2", orange, "s", "Pressure RMS")):
        values = [float(summary[("separable", "manufactured", n)][metric]) for n in SIZES]
        cg_values = [float(summary[("cg", "manufactured", n)][metric]) for n in SIZES]
        ax.loglog(SIZES, values, marker + "-", color=color, lw=2, label=label)
        ax.loglog(SIZES, cg_values, marker, mfc="white", mec=color, ms=8, ls="none")
    ax.loglog(SIZES, [0.006 * (32 / n) ** 2 for n in SIZES], ":", color=ink, lw=1.2, label="Order 2 guide")
    ax.set(xlabel="Grid n × n", ylabel="RMS error", title="Manufactured solution: grid convergence")
    ax.set_xscale("log", base=2)
    ax.set_xticks(SIZES)
    ax.set_xticklabels([str(n) for n in SIZES])
    ax.xaxis.set_minor_locator(NullLocator())
    ax.grid(which="major", color="#DCE2E6", alpha=0.8)
    ax.legend(frameon=False, fontsize=8, loc="lower left")
    ax.text(0.98, 0.96, "Filled: separable   Open: CG", transform=ax.transAxes, ha="right", va="top", fontsize=8)

    x, y, u, v, p = load_field()
    speed = np.hypot(u, v)
    ax = axes[1, 0]
    cf = ax.contourf(x, y, speed, levels=18, cmap="Blues")
    ax.streamplot(x[0], y[:, 0], u, v, color=ink, density=1.15, linewidth=0.7, arrowsize=0.7)
    ax.set(xlabel="x", ylabel="y", title="Localized forcing: numerical velocity, 64²", xlim=(0, 1), ylim=(0, 1))
    fig.colorbar(cf, ax=ax, label="Speed", shrink=0.82)

    ax = axes[1, 1]
    vmax = max(abs(float(np.min(p))), abs(float(np.max(p))))
    levels = np.linspace(-vmax, vmax, 21)
    cp = ax.contourf(x, y, p, levels=levels, cmap="RdBu_r", extend="both")
    ax.contour(x, y, p, levels=levels[::2], colors=ink, linewidths=0.35, alpha=0.45)
    ax.set(xlabel="x", ylabel="y", title="Localized forcing: numerical pressure, 64²", xlim=(0, 1), ylim=(0, 1))
    fig.colorbar(cp, ax=ax, label="Mean-zero pressure", shrink=0.82)

    note_ax.text(0.5, 0.5,
                 "Regular periodic-x channel; no cells, branched walls, wall IB, or Rapier. "
                 "Timing excludes setup and is not whole game-step time. 50 cold solves per point; p95 uses ceil((N−1)×0.95).",
                 ha="center", va="center", fontsize=8, color=ink)
    for suffix in ("png", "svg"):
        fig.savefig(RESULTS / f"overview.{suffix}", dpi=180, bbox_inches="tight", pad_inches=0.15)
    plt.close(fig)


def main() -> None:
    summary, analysis = load_and_check()
    render(summary, analysis)
    (RESULTS / "analysis.json").write_text(json.dumps(analysis, indent=2, allow_nan=False) + "\n")
    for n in SIZES:
        c = analysis["localized_cold_ms"][str(n)]
        print(f"localized {n}: CG {c['cg']['mean']:.3f}/{c['cg']['p95']:.3f} ms; "
              f"separable {c['separable']['mean']:.3f}/{c['separable']['p95']:.3f} ms (mean/p95)")
    for backend in BACKENDS:
        a = analysis["manufactured_accuracy"][backend]
        print(f"{backend}: velocity ratios {a['velocity_refinement_ratios']}; "
              f"pressure ratios {a['pressure_refinement_ratios']}")
    print("Saved overview.png, overview.svg, analysis.json")


if __name__ == "__main__":
    main()
