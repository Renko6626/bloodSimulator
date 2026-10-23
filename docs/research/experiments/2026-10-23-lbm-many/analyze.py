#!/usr/bin/env python3
"""Validate eight archived runs and summarize full-detail scaling."""
import csv
import json
import math
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt

ROOT = Path(__file__).resolve().parent
RESULTS = ROOT / "results"
COUNTS = (16, 32, 64, 128)
LAYOUTS = ("expanded", "packed")
PHASES = ("prepare_ms", "fluid_ms", "node_transfer_ms", "rapier_ms", "diagnostics_ms")
LB_DT = 1 / 2048


def read_csv(path):
    with path.open(newline="") as f:
        result = list(csv.DictReader(f))
    if not result:
        raise ValueError(f"empty CSV: {path}")
    return result


def values(rows, field):
    return [float(row[field]) for row in rows]


def mean(data):
    return sum(data) / len(data)


def p95(data):
    return sorted(data)[math.ceil(0.95 * len(data)) - 1]


def analyze_case(layout, count):
    name = f"{layout}-{count}-cached"
    directory = RESULTS / name
    log = (directory / "run.log").read_text().strip()
    if log != f"scale cached count={count} frames=360 final time=6.000000":
        raise ValueError(f"{name}: incomplete run.log: {log}")
    manifest = json.loads((directory / "manifest.json").read_text())
    width = count / 16 if layout == "expanded" else 1
    radius = 0.075 if layout == "expanded" else 0.025
    nx = 32 * width
    for key, expected in (("layout", layout), ("count", count),
                          ("domain_width", width), ("radius", radius),
                          ("nx", nx), ("ny", 32), ("h", 1 / 32),
                          ("lb_dt", LB_DT), ("duration", 6)):
        if manifest[key] != expected:
            raise ValueError(f"{name}: manifest {key}={manifest[key]} expected {expected}")
    frames = read_csv(directory / "frames.csv")
    if len(frames) != 360:
        raise ValueError(f"{name}: {len(frames)} frames")
    for index, row in enumerate(frames, 1):
        if (row["scene"], int(row["count"]), row["mode"], int(row["frame"])) != (
                "scale", count, "cached", index):
            raise ValueError(f"{name}: frame identity {index}")
        if abs(float(row["t"]) - index / 60) > 5e-10:
            raise ValueError(f"{name}: frame time {index}")
        for key, value in row.items():
            if key in ("scene", "mode"):
                continue
            number = float(value)
            if key == "closest_intercell_node_distance" and math.isinf(number) and number > 0:
                continue
            if not math.isfinite(number):
                raise ValueError(f"{name}: nonfinite {key} at frame {index}")
    steps = values(frames, "fluid_steps")
    if sum(steps) != 12288:
        raise ValueError(f"{name}: LB step count {sum(steps)}")
    cumulative = 0
    for index, (step, row) in enumerate(zip(steps, frames), 1):
        cumulative += step
        clock = cumulative * LB_DT - index / 60
        if abs(clock - float(row["fluid_time_mismatch"])) > 5e-10:
            raise ValueError(f"{name}: LB clock at frame {index}")
    rho_min = min(values(frames, "min_rho"))
    if rho_min <= 0 or min(values(frames, "min_area")) <= 0:
        raise ValueError(f"{name}: nonpositive density or area")
    impulse_max = max(values(frames, "exchange_impulse_mismatch"))
    if impulse_max > 1e-6:
        raise ValueError(f"{name}: impulse mismatch {impulse_max}")
    area_error = max(values(frames, "max_area_rel_error"))
    if area_error >= 1:
        raise ValueError(f"{name}: area relative error {area_error}")
    outline = read_csv(directory / "outlines.csv")
    snapshot_frames = list(range(30 if count == 128 else 6, 361,
                                 30 if count == 128 else 6))
    expected = [(f, c, n) for f in snapshot_frames for c in range(count) for n in range(12)]
    actual = [(int(row["frame"]), int(row["cell"]), int(row["node"])) for row in outline]
    if actual != expected:
        raise ValueError(f"{name}: outline snapshot coverage")
    for row in outline:
        if not math.isfinite(float(row["x"])) or not math.isfinite(float(row["y"])):
            raise ValueError(f"{name}: nonfinite outline")
    timing = frames[30:]
    whole = values(timing, "whole_ms")
    phases = {key.removesuffix("_ms") + "_mean_ms": mean(values(timing, key)) for key in PHASES}
    distances = [v for v in values(frames, "closest_intercell_node_distance") if math.isfinite(v)]
    if not distances or min(distances) <= 0:
        raise ValueError(f"{name}: sampled node distance")
    if count == 16 and len(distances) != 360:
        raise ValueError(f"{name}: missing 16-cell node distance")
    if count > 16 and len(distances) != 13:
        raise ValueError(f"{name}: sampled node distance count {len(distances)}")
    return {
        "layout": layout, "count": count, "domain_width": width, "radius": radius,
        "nx": nx, "ny": 32, "h": 1 / 32, "lb_dt": LB_DT,
        "frames": 360, "warmup_frames_excluded": 30, "timed_frames": 330,
        "lb_steps": int(sum(steps)), "final_clock_error_s": float(frames[-1]["fluid_time_mismatch"]),
        "max_abs_clock_error_s": max(abs(v) for v in values(frames, "fluid_time_mismatch")),
        "whole_mean_ms": mean(whole), "whole_p95_ms": p95(whole),
        "whole_minus_diagnostics_mean_ms": mean([float(r["whole_ms"]) - float(r["diagnostics_ms"]) for r in timing]),
        **phases,
        "exchange_impulse_mismatch_max": impulse_max,
        "rho_min": rho_min, "rho_max": max(values(frames, "max_rho")),
        "max_relative_fluid_mass_error": max(abs(v / width - 1) for v in values(frames, "total_mass")),
        "max_area_relative_error": area_error,
        "frames_with_solver_contact": sum(float(r["solver_contacts_sum"]) > 0 for r in frames),
        "max_solver_contacts_per_substep": int(max(values(frames, "solver_contacts_max"))),
        "min_sampled_intercell_node_distance": min(distances),
        "sampled_intercell_node_distance_frames": len(distances),
    }


def plot(rows):
    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 10,
                         "axes.spines.top": False, "axes.spines.right": False})
    fig, axes = plt.subplots(2, 2, figsize=(12, 8), layout="constrained")
    colors = ("#6599a5", "#d49654", "#9179a5", "#ba6969", "#898e91")
    for i, layout in enumerate(LAYOUTS):
        group = [r for r in rows if r["layout"] == layout]
        x = [r["count"] for r in group]
        ax = axes[i, 0]
        ax.plot(x, [r["whole_mean_ms"] for r in group], "o-", color="#216472", label="whole mean")
        ax.plot(x, [r["whole_p95_ms"] for r in group], "s--", color="#b26337", label="whole p95")
        ax.set_xticks(x)
        ax.set_xlabel("Cell count")
        ax.set_ylabel("Time per frame (ms)")
        ax.grid(axis="y", alpha=0.2)
        ax.legend(frameon=False)
        ax.set_title(f"{layout}: r={0.075 if layout == 'expanded' else 0.025}; "
                     + ("Nx=32–256, Ny=32" if layout == "expanded" else "Nx=Ny=32"))
        ax = axes[i, 1]
        bottom = [0.0] * len(group)
        for key, color in zip(PHASES, colors):
            vals = [r[key.removesuffix("_ms") + "_mean_ms"] for r in group]
            ax.bar(x, vals, bottom=bottom, width=8, color=color, label=key.removesuffix("_ms"))
            bottom = [a + b for a, b in zip(bottom, vals)]
        ax.set_xticks(x)
        ax.set_xlabel("Cell count")
        ax.set_ylabel("Mean measured phase time per frame (ms)")
        ax.set_title("Stage means, frames 31–360")
        ax.legend(frameon=False, fontsize=8, ncol=2)
    fig.suptitle("Full-detail scaling at fixed h=1/32 and LB dt=1/2048\n"
                 "Expanded and packed layouts are different physical cases", fontsize=13)
    fig.savefig(RESULTS / "scaling.png", dpi=160)
    plt.close(fig)


def main():
    rows = [analyze_case(layout, count) for layout in LAYOUTS for count in COUNTS]
    (RESULTS / "summary.json").write_text(json.dumps({"cases": rows}, indent=2) + "\n")
    with (RESULTS / "summary.csv").open("w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    plot(rows)
    for row in rows:
        print(f"{row['layout']:8} {row['count']:3} whole {row['whole_mean_ms']:.3f} "
              f"p95 {row['whole_p95_ms']:.3f} ms/frame; "
              f"contacts {row['frames_with_solver_contact']} frames")


if __name__ == "__main__":
    main()
