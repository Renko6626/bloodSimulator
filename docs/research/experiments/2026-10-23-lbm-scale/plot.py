#!/usr/bin/env python3
"""Validate archived fixed-grid runs and plot their measured results."""
import csv
import json
import math
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

ROOT = Path(__file__).resolve().parent
RESULTS = ROOT / "results"
OLD = ROOT.parent / "2026-10-23-lbm-dynamics" / "results" / "rapier"
LB_DT = 1 / 2048
RUNS = [(s, n, m) for s, n in (("reversal", 1), ("squeeze", 1),
                               ("scale", 1), ("scale", 4), ("scale", 16))
        for m in ("baseline", "cached")] + [("counterflow", 16, "cached")]
TIMES = ("prepare_ms", "fluid_ms", "node_transfer_ms", "rapier_ms",
         "diagnostics_ms", "whole_ms")
ID = ("scene", "count", "mode", "frame", "t", "cell", "node")


def rows(path):
    with path.open(newline="") as f:
        data = list(csv.DictReader(f))
    if not data:
        raise ValueError(f"missing or empty data: {path}")
    return data


def col(data, key):
    return np.array([float(r[key]) for r in data])


def rank95(a):
    a = sorted(a)
    return float(a[math.ceil(0.95 * len(a)) - 1])


def describe(a):
    return {"mean_ms": float(np.mean(a)), "p95_ms": rank95(a)}


def numeric_check(data, ignore=()):
    for key in data[0]:
        if key in ("scene", "mode", "scenario") or key in ignore:
            continue
        a = col(data, key)
        if not np.all(np.isfinite(a)):
            raise ValueError(f"nonfinite {key}")


def load_run(scene, count, mode):
    d = RESULTS / f"{scene}-{count}-{mode}"
    return rows(d / "frames.csv"), rows(d / "outlines.csv")


def check_run(name, frames, outline, count):
    numeric_check(frames, ignore=("closest_intercell_node_distance",))
    numeric_check(outline)
    if len(frames) != 360 or not np.array_equal(col(frames, "frame"), np.arange(1, 361)):
        raise ValueError(f"{name}: frames incomplete")
    if not np.allclose(col(frames, "t"), col(frames, "frame") / 60, atol=5e-10, rtol=0):
        raise ValueError(f"{name}: frame time mismatch")
    steps = col(frames, "fluid_steps")
    cumulative = np.cumsum(steps)
    if int(cumulative[-1]) != 12288 or not np.allclose(
            cumulative * LB_DT - col(frames, "frame") / 60,
            col(frames, "fluid_time_mismatch"), atol=5e-10, rtol=0):
        raise ValueError(f"{name}: LBM clock mismatch")
    if np.max(np.abs(col(frames, "fluid_time_mismatch"))) >= LB_DT + 1e-12:
        raise ValueError(f"{name}: drift above one LBM step")
    if np.min(col(frames, "min_rho")) <= 0 or np.min(col(frames, "min_area")) <= 0:
        raise ValueError(f"{name}: nonpositive density or area")
    if np.max(col(frames, "exchange_impulse_mismatch")) > 1e-6:
        raise ValueError(f"{name}: body-fluid impulse mismatch")
    if np.any(col(frames, "max_area_rel_error") >= 1):
        raise ValueError(f"{name}: area deviation >= 100%")
    if count == 1:
        if not np.all(np.isinf(col(frames, "closest_intercell_node_distance"))):
            raise ValueError(f"{name}: single body should have undefined intercell distance")
    elif not np.all(np.isfinite(col(frames, "closest_intercell_node_distance"))):
        raise ValueError(f"{name}: intercell distance nonfinite")
    expected_frames = np.arange(6, 361, 6)
    if len(outline) != 60 * count * 12:
        raise ValueError(f"{name}: outline row count")
    key = [(int(r["frame"]), int(r["cell"]), int(r["node"])) for r in outline]
    expected = [(f, c, i) for f in expected_frames for c in range(count) for i in range(12)]
    if key != expected:
        raise ValueError(f"{name}: outline snapshots misaligned")
    xy = np.array([[float(r["x"]), float(r["y"])] for r in outline]).reshape(60, count, 12, 2)
    signed = 0.5 * np.sum(xy[..., 0] * np.roll(xy[..., 1], -1, axis=2) -
                          xy[..., 1] * np.roll(xy[..., 0], -1, axis=2), axis=2)
    if np.any(signed * signed[0:1] <= 0):
        raise ValueError(f"{name}: saved outline area flipped")
    return {
        "frames": len(frames), "duration_s": float(col(frames, "t")[-1]),
        "lb_steps": int(cumulative[-1]),
        "max_abs_clock_error_s": float(np.max(np.abs(col(frames, "fluid_time_mismatch")))),
        "max_exchange_impulse_mismatch": float(np.max(col(frames, "exchange_impulse_mismatch"))),
        "rho_min": float(np.min(col(frames, "min_rho"))),
        "rho_max": float(np.max(col(frames, "max_rho"))),
        "total_mass_min": float(np.min(col(frames, "total_mass"))),
        "total_mass_max": float(np.max(col(frames, "total_mass"))),
        "min_area": float(np.min(col(frames, "min_area"))),
        "max_area_relative_error": float(np.max(col(frames, "max_area_rel_error"))),
        "max_node_speed": float(np.max(col(frames, "max_node_speed"))),
        "max_node_jump": float(np.max(col(frames, "max_node_jump"))),
        "max_solver_contacts_per_substep": int(np.max(col(frames, "solver_contacts_max"))),
        "frames_with_solver_contact": int(np.count_nonzero(col(frames, "solver_contacts_sum"))),
        "timing_excluding_first_30_frames": {
            **{k: describe(col(frames, k)[30:]) for k in TIMES},
            "prepare_plus_fluid_ms": describe((col(frames, "prepare_ms") +
                                                col(frames, "fluid_ms"))[30:]),
        },
    }


def compare(a, b, kind):
    if len(a) != len(b) or a[0].keys() != b[0].keys():
        raise ValueError(f"{kind}: A/B schema or row count mismatch")
    ignored = {"mode", *TIMES} if kind == "frames" else {"mode"}
    for i, (x, y) in enumerate(zip(a, b)):
        for k in x:
            if k not in ignored and x[k] != y[k]:
                raise ValueError(f"{kind}: A/B {k} differs at row {i + 1}: {x[k]} vs {y[k]}")
    return {"rows": len(a), "all_nontiming_columns_identical": True}


def compare_old(scene, new):
    old = rows(OLD / f"{scene}.csv")
    keymap = {"min_area": "area", "max_area_rel_error": "area_rel_error"}
    excluded = {"fluid_ms", "rapier_ms", "whole_ms"}
    fields = [k for k in old[0] if k not in excluded]
    if len(new) != len(old):
        raise ValueError(f"{scene}: old frame count mismatch")
    diffs = {}
    # Cross-harness comparison is descriptive: the new f32 radius * 0.1 is one ULP
    # above the old literal f32 0.015 for the soft-body particle/contact radius.
    for k in fields:
        other = next((nk for nk, ok in keymap.items() if ok == k), k)
        a, b = col(old, k), col(new, other)
        delta = float(np.max(np.abs(a - b)))
        diffs[k] = delta
    if scene == "reversal" and any(d != 0 for d in diffs.values()):
        raise ValueError("reversal: non-contact reference changed")
    return {"physical_columns_checked": fields, "maximum_absolute_differences": diffs,
            "exact_to_csv_precision": all(d == 0 for d in diffs.values()),
            "comparison": "descriptive cross-harness comparison; not a pass/fail tolerance",
            "known_input_difference": "new f32 (0.15 * 0.1) contact radius bits 0x3c75c290 vs old literal 0.015 bits 0x3c75c28f"}


def segment_distance(p, q, r, s):
    # Vectorized over all pairs of polygon edges. Intersections have zero distance.
    u, v = q - p, s - r
    def point_segment(a, b, c):
        w = c - b
        t = np.clip(np.sum((a - b) * w, axis=-1) / np.maximum(np.sum(w * w, axis=-1), 1e-30), 0, 1)
        return np.linalg.norm(a - (b + t[..., None] * w), axis=-1)
    ds = np.minimum.reduce((point_segment(p, r, s), point_segment(q, r, s),
                            point_segment(r, p, q), point_segment(s, p, q)))
    def cross(a, b):
        return a[..., 0] * b[..., 1] - a[..., 1] * b[..., 0]
    den = cross(u, v)
    nonparallel = np.abs(den) > 1e-14
    t = np.divide(cross(r - p, v), den, out=np.zeros_like(den), where=nonparallel)
    w = np.divide(cross(r - p, u), den, out=np.zeros_like(den), where=nonparallel)
    ds[nonparallel & (t >= 0) & (t <= 1) & (w >= 0) & (w <= 1)] = 0
    return float(np.min(ds))


def outline_distances(outline):
    a = np.array([[float(r["x"]), float(r["y"])] for r in outline]).reshape(60, 16, 12, 2)
    result = []
    for snapshot in a:
        best = math.inf
        for i in range(16):
            p = snapshot[i][:, None, :]
            q = np.roll(snapshot[i], -1, axis=0)[:, None, :]
            for j in range(i + 1, 16):
                r = snapshot[j][None, :, :]
                s = np.roll(snapshot[j], -1, axis=0)[None, :, :]
                best = min(best, segment_distance(p, q, r, s))
        result.append(best)
    return np.array(result)


def repeat16(initial):
    directory = RESULTS / "repeat16"
    order = json.loads((directory / "order.json").read_text())
    per_run = {}
    pooled = {}
    for mode in ("baseline", "cached"):
        source = initial[f"scale-16-{mode}"][0]
        samples = [source]
        for label in ("reverse", "forward"):
            current = rows(directory / f"{label}-{mode}" / "frames.csv")
            compare(source, current, "frames")
            samples.append(current)
        for label, sample in zip(("initial", "reverse", "forward"), samples):
            phase = {k: describe(col(sample, k)[30:]) for k in TIMES}
            phase["prepare_plus_fluid_ms"] = describe(
                (col(sample, "prepare_ms") + col(sample, "fluid_ms"))[30:])
            per_run[f"{label}-{mode}"] = phase
        phase = {k: describe(np.concatenate([col(s, k)[30:] for s in samples])) for k in TIMES}
        phase["prepare_plus_fluid_ms"] = describe(np.concatenate(
            [(col(s, "prepare_ms") + col(s, "fluid_ms"))[30:] for s in samples]))
        pooled[mode] = phase
    return {"order": order, "warmup_frames_excluded_per_run": 30,
            "samples_per_mode": 990, "repeated_physics_exactly_matches_initial": True,
            "per_run": per_run, "pooled": pooled,
            "pooled_whole_ratio_baseline_over_cached":
                pooled["baseline"]["whole_ms"]["mean_ms"] / pooled["cached"]["whole_ms"]["mean_ms"]}


def plot(data, summaries, distances, repeat):
    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10,
                         "axes.spines.top": False, "axes.spines.right": False})
    fig, axs = plt.subplots(2, 2, figsize=(13, 9), layout="constrained")
    ax = axs[0, 0]
    for mode, color in (("baseline", "#526889"), ("cached", "#d07936")):
        vals = [summaries[f"scale-{n}-{mode}"]["timing_excluding_first_30_frames"]["whole_ms"]["mean_ms"] for n in (1, 4)]
        vals.append(repeat["pooled"][mode]["whole_ms"]["mean_ms"])
        ax.plot((1, 4, 16), vals, "o-", color=color, label=mode)
    ax.set(xlabel="Cells (16: three runs pooled), fixed n=32 and radius=0.075", ylabel="Mean frame time (ms)", title="Scale: measured whole-frame time")
    ax.set_xticks((1, 4, 16)); ax.set_ylim(bottom=0); ax.legend(frameon=False)
    ax = axs[0, 1]
    phases = ("prepare_ms", "fluid_ms", "node_transfer_ms", "rapier_ms", "diagnostics_ms")
    colors = ("#b7c4cf", "#356da2", "#88ad95", "#ce9358", "#947da6")
    bottoms = np.zeros(3)
    for phase, color in zip(phases, colors):
        vals = np.array([summaries[f"scale-{n}-cached"]["timing_excluding_first_30_frames"][phase]["mean_ms"] for n in (1, 4)] +
                        [repeat["pooled"]["cached"][phase]["mean_ms"]])
        ax.bar((1, 4, 16), vals, bottom=bottoms, color=color, label=phase.replace("_ms", ""), width=0.8)
        bottoms += vals
    ax.set(xlabel="Cells (16: three runs pooled)", ylabel="Mean phase time (ms)", title="Cached mode: measured phases")
    ax.set_xticks((1, 4, 16)); ax.legend(frameon=False, fontsize=8)
    cf = data["counterflow-16-cached"][0]
    ax = axs[1, 0]
    t = col(cf, "t")
    ax.plot(t, col(cf, "max_area_rel_error"), color="#356da2", label="Max area relative error")
    ax.set(xlabel="Time (s)", ylabel="Maximum area relative error", title="Counterflow: deformation and solver contacts")
    twin = ax.twinx()
    twin.plot(t, col(cf, "solver_contacts_sum"), color="#ce784a", alpha=0.7, label="Solver contacts / frame")
    twin.set_ylabel("Full-world solver contacts / frame")
    lines = ax.lines + twin.lines
    ax.legend(lines, [x.get_label() for x in lines], loc="upper left", frameon=False, fontsize=8)
    ax = axs[1, 1]
    a = np.array([[float(r["x"]), float(r["y"])] for r in data["counterflow-16-cached"][1]]).reshape(60, 16, 12, 2)
    for snap, color, alpha in ((a[0], "#9ba8b5", 0.75), (a[-1], "#bc633b", 0.9)):
        for cell in snap:
            closed = np.vstack((cell, cell[0]))
            ax.plot(closed[:, 0], closed[:, 1], color=color, alpha=alpha, lw=1)
    ax.plot([], [], color="#9ba8b5", label="t=0.1 s")
    ax.plot([], [], color="#bc633b", label="t=6 s")
    ax.set(xlabel="x", ylabel="y", title="Counterflow: saved cell outlines", xlim=(0, 1), ylim=(0, 1))
    ax.set_aspect("equal"); ax.legend(frameon=False, fontsize=8)
    fig.suptitle("Fixed-grid LBM / Rapier scale study", fontsize=15)
    fig.savefig(RESULTS / "overview.png", dpi=170)
    fig.savefig(RESULTS / "overview.svg")
    plt.close(fig)


def main():
    data = {}; summary = {}
    for scene, count, mode in RUNS:
        name = f"{scene}-{count}-{mode}"
        frame, outline = load_run(scene, count, mode)
        data[name] = (frame, outline)
        summary[name] = check_run(name, frame, outline, count)
    comparisons = {}
    for scene, count in (("reversal", 1), ("squeeze", 1), ("scale", 1), ("scale", 4), ("scale", 16)):
        a, b = data[f"{scene}-{count}-baseline"], data[f"{scene}-{count}-cached"]
        comparisons[f"{scene}-{count}"] = {"frames": compare(a[0], b[0], "frames"),
                                           "outlines": compare(a[1], b[1], "outlines")}
    old = {scene: compare_old(scene, data[f"{scene}-1-baseline"][0]) for scene in ("reversal", "squeeze")}
    cf, outline = data["counterflow-16-cached"]
    contact = col(cf, "solver_contacts_sum") > 0
    dist = outline_distances(outline)
    cf_summary = {
        "first_full_world_solver_contact_frame": int(col(cf, "frame")[np.flatnonzero(contact)[0]]) if np.any(contact) else None,
        "frames_with_full_world_solver_contact": int(np.sum(contact)),
        "minimum_node_to_node_distance": float(np.min(col(cf, "closest_intercell_node_distance"))),
        "minimum_saved_outline_to_outline_distance": float(np.min(dist)),
        "minimum_saved_outline_distance_frame": int(6 * (np.argmin(dist) + 1)),
        "saved_outline_interval_frames": 6,
    }
    gains = {}
    for n in (1, 4, 16):
        a = summary[f"scale-{n}-baseline"]["timing_excluding_first_30_frames"]
        b = summary[f"scale-{n}-cached"]["timing_excluding_first_30_frames"]
        gains[str(n)] = {k: {"baseline_mean_ms": a[k]["mean_ms"], "cached_mean_ms": b[k]["mean_ms"],
                             "ratio_baseline_over_cached": a[k]["mean_ms"] / b[k]["mean_ms"]}
                         for k in ("prepare_plus_fluid_ms", "whole_ms")}
    repeated = repeat16(data)
    output = {"runs": summary, "mode_comparisons": comparisons, "reference_archive_comparison": old,
              "repeat16": repeated,
              "scale_speedups": gains, "counterflow_contact": cf_summary,
              "interpretation_limits": [
                  "Scale 1/4/16 uses one unit domain, n=32 and radius=0.075; reference single-cell scenes use radius=0.15.",
                  "Solver contacts are full-world counters, possibly including self or plate contact; they are not cell-cell counts.",
                  "A one-cell intercell distance is undefined (CSV infinity), not an instability.",
                  "Timing excludes 30 warmup frames; p95 is nearest-rank and phase p95 values are not additive.",
                  "Baseline marker preparation is inside fluid_ms. Compare prepare_ms+fluid_ms or whole_ms.",
                  "whole_ms includes research O(N^2) node-pair distance diagnostics and excludes CSV writing.",
                  "Runs were sequential and nonexclusive; measured speedups are observations, not stable performance guarantees.",
                  "The 16-cell chart point pools three runs per mode; 1- and 4-cell points use one run per mode.",
                  "Outline distances are computed from six-frame snapshots and ignore collision-skin thickness."],
              "source": "results/*/{frames,outlines}.csv; prior 2026-10-23-lbm-dynamics/results/rapier/{reversal,squeeze}.csv"}
    (RESULTS / "analysis.json").write_text(json.dumps(output, indent=2, ensure_ascii=False) + "\n")
    plot(data, summary, dist, repeated)
    print(json.dumps({"comparisons": list(comparisons), "speedups": gains,
                      "repeat16": repeated, "counterflow": cf_summary}, indent=2))


if __name__ == "__main__":
    main()
