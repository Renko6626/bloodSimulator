"""Audit the repeated coupling probes and draw their numerical fields.

Run after both sample and summary CSVs are complete:
    python3 docs/research/experiments/2026-10-23-coupling/plot.py
"""

from __future__ import annotations

import csv
import json
import math
from collections import defaultdict
from pathlib import Path
from statistics import mean

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.ticker import FixedFormatter, FixedLocator, NullLocator


ROOT = Path(__file__).resolve().parent
RESULTS = ROOT / "results"
TARGET_SPEED = 0.1
TIMES = {
    "volume": ("solve_ms",),
    "markers": ("assemble_ms", "factor_ms", "final_fluid_ms", "total_ms"),
}
KEYS = {
    "volume": ("scene", "n", "kappa"),
    "markers": ("kind", "shape", "n", "m", "parameter_kind", "parameter"),
}


def read_csv(name: str) -> list[dict[str, str]]:
    with (RESULTS / name).open(newline="") as stream:
        return list(csv.DictReader(stream))


def key(row: dict[str, str], fields: tuple[str, ...]) -> tuple[str, ...]:
    return tuple(row[field] for field in fields)


def p95(values: list[float]) -> float:
    ordered = sorted(values)
    return ordered[math.ceil((len(ordered) - 1) * 0.95)]


def audit(kind: str) -> tuple[list[dict], list[dict[str, str]]]:
    samples = read_csv(f"{kind}-samples.csv")
    summaries = read_csv(f"{kind}-summary.csv")
    groups: dict[tuple[str, ...], list[dict[str, str]]] = defaultdict(list)
    for row in samples:
        groups[key(row, KEYS[kind])].append(row)
    assert len(groups) == (12 if kind == "volume" else 24), (kind, len(groups))
    assert len(summaries) == len(groups)
    audited = []
    for summary in summaries:
        case = key(summary, KEYS[kind])
        group = groups.pop(case)
        assert len(group) == 20 and int(summary["runs"]) == 20, (kind, case)
        assert sorted(int(row["rep"]) for row in group) == list(range(20))
        for field in TIMES[kind]:
            values = [float(row[field]) for row in group]
            expected = {field + "_mean": mean(values), field + "_p95": p95(values)}
            for name, value in expected.items():
                assert math.isclose(float(summary[name]), value, rel_tol=1e-10, abs_tol=1e-9), (
                    kind, case, name, summary[name], value
                )
        # The runner copies the final row's diagnostics; check it really represents every run.
        for field, reported in summary.items():
            if field in KEYS[kind] or field == "runs" or any(
                field.startswith(time + "_") for time in TIMES[kind]
            ):
                continue
            for row in group:
                actual = row[field]
                if reported == "NaN" and actual == "NaN":
                    continue
                try:
                    assert math.isclose(float(actual), float(reported), rel_tol=1e-9, abs_tol=1e-12)
                except ValueError:
                    assert actual == reported
        audited.append({
            "case": {field: summary[field] for field in KEYS[kind]},
            "runs": len(group),
            "times_ms": {
                field: {"mean": mean([float(r[field]) for r in group]),
                        "p95": p95([float(r[field]) for r in group])}
                for field in TIMES[kind]
            },
        })
    assert not groups
    return audited, summaries


def field_grid(name: str, n: int) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
    rows = read_csv("fields/" + name)
    assert len(rows) == n * n, name
    x = np.array([float(row["x"]) for row in rows]).reshape(n, n)
    y = np.array([float(row["y"]) for row in rows]).reshape(n, n)
    u = np.array([float(row["u"]) for row in rows]).reshape(n, n)
    v = np.array([float(row["v"]) for row in rows]).reshape(n, n)
    assert np.all(np.diff(x[0]) > 0) and np.all(np.diff(y[:, 0]) > 0)
    return x[0], y[:, 0], u, v


def draw() -> None:
    volume_audit, volume = audit("volume")
    markers_audit, markers = audit("markers")
    blob = [r for r in markers if r["shape"] == "blob" and r["n"] == "32"]
    channel = sorted(
        (r for r in volume if r["scene"] == "channel" and r["n"] == "32"),
        key=lambda r: float(r["kappa"]),
    )
    assert [float(r["kappa"]) for r in channel] == [256, 1024, 4096]
    assert {float(r["parameter"]) for r in blob} == {0.25, 1, 4, 16}

    for row in markers:
        assert float(row["force_balance_abs"]) < 1e-9
        assert float(row["div_rms"]) < 1e-8
        assert float(row["momentum_rms"]) < 1e-10
        if row["kind"] == "self":
            assert float(row["force_law_rms"]) < 1e-9
            assert float(row["mobility_antisym_rel"]) < 1e-6
        if row["shape"] == "blob":
            assert math.isnan(float(row["normal_abs_integral"]))
            assert math.isnan(float(row["tangent_abs_integral"]))
    for row in volume:
        assert float(row["mom_rms"]) < 1e-8
        assert float(row["div_rms"]) < 1e-8
        if row["scene"] == "disk-moving":
            assert float(row["reaction_x"]) < 0

    lookup = {(r["kind"], int(r["m"]), float(r["parameter"])): r for r in blob}
    blob_table = []
    for m in (1, 4):
        for gamma in (0.25, 1, 4, 16):
            self_row = lookup[("self", m, gamma)]
            lag = lookup[("lagged", m, gamma)]
            blob_table.append({
                "markers": m, "gamma_total": gamma,
                "self_law_rms_velocity": float(self_row["force_law_rms"]),
                "lagged_law_rms_velocity": float(lag["force_law_rms"]),
                "lagged_law_rms_over_target_speed": float(lag["force_law_rms"]) / TARGET_SPEED,
                "self_max_sampled_speed_over_target": float(self_row["max_sampled_speed_over_target"]),
                "lagged_max_sampled_speed_over_target": float(lag["max_sampled_speed_over_target"]),
                "self_reaction_x": float(self_row["reaction_x"]),
                "lagged_reaction_x": float(lag["reaction_x"]),
                "self_cold_total_ms_mean": float(self_row["total_ms_mean"]),
                "self_cold_total_ms_p95": float(self_row["total_ms_p95"]),
                "lagged_warm_update_ms_mean": float(lag["total_ms_mean"]),
                "lagged_warm_update_ms_p95": float(lag["total_ms_p95"]),
            })
    analysis = {
        "method": "20 runs per case; p95 is sorted samples at ceil((N-1)*0.95), index 19 for N=20",
        "timing_scope": "self cold total includes mobility assembly; lagged warm update reuses setup, so timings are not a production speedup ratio",
        "audit": {"volume_cases": volume_audit, "marker_cases": markers_audit},
        "max_observed_residuals": {
            "volume_momentum_rms": max(float(r["mom_rms"]) for r in volume),
            "volume_divergence_rms": max(float(r["div_rms"]) for r in volume),
            "marker_self_force_law_rms_velocity": max(float(r["force_law_rms"]) for r in markers if r["kind"] == "self"),
            "marker_divergence_rms": max(float(r["div_rms"]) for r in markers),
            "marker_momentum_rms": max(float(r["momentum_rms"]) for r in markers),
            "marker_force_balance_abs": max(float(r["force_balance_abs"]) for r in markers),
        },
        "blob_n32": blob_table,
        "channel_n32": [{
            "kappa": float(r["kappa"]), "normal_rms": float(r["normal_rms"]),
            "solve_ms_mean": float(r["solve_ms_mean"]), "solve_ms_p95": float(r["solve_ms_p95"]),
        } for r in channel],
        "channel_fixed_kappa_1024": [{
            "n": int(r["n"]), "normal_rms": float(r["normal_rms"]),
            "solve_ms_mean": float(r["solve_ms_mean"]),
        } for r in volume if r["scene"] == "channel" and float(r["kappa"]) == 1024],
        "circle_n32_kappa32": [{
            "m": int(r["m"]), "normal_abs_integral": float(r["normal_abs_integral"]),
            "slip_rms": float(r["slip_rms"]),
        } for r in markers if r["shape"] == "circle" and r["kind"] == "self" and r["n"] == "32"],
    }
    RESULTS.mkdir(exist_ok=True)
    (RESULTS / "analysis.json").write_text(json.dumps(analysis, indent=2) + "\n")

    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10,
                         "axes.spines.top": False, "axes.spines.right": False,
                         "axes.titleweight": "bold", "savefig.facecolor": "white"})
    fig, axes = plt.subplots(2, 2, figsize=(13.3, 9.5), layout="constrained")
    fig.suptitle("Steady fluid-coupling probes", fontsize=17, fontweight="bold")
    blue, orange = "#275d8c", "#c05b35"

    ax = axes[0, 0]
    for m, color, symbol in ((1, blue, "o"), (4, orange, "s")):
        rows = [r for r in blob_table if r["markers"] == m]
        ax.plot([r["gamma_total"] for r in rows],
                [r["lagged_law_rms_over_target_speed"] for r in rows],
                marker=symbol, color=color, linewidth=2, label=f"{m} coupling point{'s' if m > 1 else ''}")
    ax.set(xscale="log", xticks=[0.25, 1, 4, 16], xlabel="Total blob resistance Γ",
           ylabel="Lagged force-law RMS / 0.1", title="A  One-step closure error")
    ax.set_xticklabels(["0.25", "1", "4", "16"])
    ax.grid(axis="y", alpha=0.22)
    ax.legend(frameon=False, fontsize=9)

    ax = axes[0, 1]
    kap = [float(r["kappa"]) for r in channel]
    normal = [float(r["normal_rms"]) for r in channel]
    times = [float(r["solve_ms_p95"]) for r in channel]
    ax.plot(kap, normal, "o-", color=blue, linewidth=2, label="Normal RMS")
    ax.set(xscale="log", xticks=kap, xlabel="Volume penalty κ",
           ylabel="Boundary-normal velocity RMS", title="B  Contraction: penalty and cost")
    ax.set_xticklabels(["256", "1024", "4096"])
    ax.tick_params(axis="y", colors=blue)
    ax2 = ax.twinx()
    ax2.plot(kap, times, "s--", color=orange, linewidth=2, label="Complete solve p95")
    ax2.set_ylabel("Complete solve p95 (ms)", color=orange)
    ax2.tick_params(axis="y", colors=orange)
    ax.xaxis.set_major_locator(FixedLocator(kap))
    ax.xaxis.set_major_formatter(FixedFormatter(["256", "1024", "4096"]))
    ax.xaxis.set_minor_locator(NullLocator())
    ax.grid(axis="y", alpha=0.22)

    ax = axes[1, 0]
    x, y, u, v = field_grid("volume_channel_32_1024.csv", 32)
    xx, yy = np.meshgrid(x, y)
    lower = 0.12 + 0.04 * np.sin(2 * np.pi * xx)
    upper = 0.88 - 0.08 * np.sin(2 * np.pi * xx)
    solid = (yy < lower) | (yy > upper)
    speed = np.hypot(u, v)
    image = ax.pcolormesh(xx, yy, np.ma.array(speed, mask=solid), cmap="Blues", shading="auto")
    ax.streamplot(x, y, np.ma.array(u, mask=solid), np.ma.array(v, mask=solid),
                  color="#263e52", density=1.0, linewidth=0.65, arrowsize=0.7)
    curve_x = np.linspace(0, 1, 600)
    ax.fill_between(curve_x, 0, 0.12 + 0.04 * np.sin(2 * np.pi * curve_x), color="#e9e4dd")
    ax.fill_between(curve_x, 0.88 - 0.08 * np.sin(2 * np.pi * curve_x), 1, color="#e9e4dd")
    ax.plot(curve_x, 0.12 + 0.04 * np.sin(2 * np.pi * curve_x), color="#766f68", lw=1)
    ax.plot(curve_x, 0.88 - 0.08 * np.sin(2 * np.pi * curve_x), color="#766f68", lw=1)
    ax.set(xlim=(0, 1), ylim=(0, 1), xlabel="x", ylabel="y", aspect="equal",
           title="C  Contraction flow: n=32, κ=1024")
    fig.colorbar(image, ax=ax, fraction=0.045, pad=0.025, label="Cell-center speed")

    ax = axes[1, 1]
    x, y, u, v = field_grid("field_blob_n32_m4_p4.csv", 32)
    speed = np.hypot(u, v)
    image = ax.pcolormesh(x, y, speed, cmap="Blues", shading="auto")
    ax.streamplot(x, y, u, v, color="#263e52", density=1.0,
                  linewidth=0.65, arrowsize=0.7)
    point_x = [0.575, 0.5, 0.425, 0.5]
    point_y = [0.5, 0.575, 0.5, 0.425]
    ax.scatter(point_x, point_y, s=62, marker="o", color=orange,
               edgecolor="white", linewidth=1.2, zorder=10, label="4 coupling points")
    ax.set(xlim=(0, 1), ylim=(0, 1), xlabel="x", ylabel="y", aspect="equal",
           title="D  Four-point blob flow: n=32, Γ=4")
    ax.legend(frameon=True, facecolor="white", edgecolor="none", loc="upper right", fontsize=9)
    fig.colorbar(image, ax=ax, fraction=0.045, pad=0.025, label="Cell-center speed")

    fig.savefig(RESULTS / "overview.png", dpi=180)
    fig.savefig(RESULTS / "overview.svg")
    plt.close(fig)
    print("Audited 12 volume cases and 12 marker cases (self and lagged), 20 samples each; wrote analysis.json, overview.png, overview.svg")


if __name__ == "__main__":
    draw()
