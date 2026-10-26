#!/usr/bin/env python3
"""Summarize every complete raw trajectory without selecting a fastest run."""
import argparse
import csv
import json
import math
import pathlib
import statistics

HERE = pathlib.Path(__file__).resolve().parent


def percentile_nearest(values, percent):
    values = sorted(values)
    return values[max(0, math.ceil(percent / 100 * len(values)) - 1)]


def describe(values):
    return {"mean": statistics.fmean(values), "p95": percentile_nearest(values, 95),
            "max": max(values)}


def read(path):
    with path.open(newline="") as stream:
        return list(csv.DictReader(stream))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", type=pathlib.Path, default=HERE / "results")
    args = parser.parse_args()
    out = {}
    for case in sorted(args.results.iterdir()):
        path = case / "frames.csv"
        if not path.is_file():
            continue
        rows = read(path)
        if not rows:
            raise ValueError(f"empty frames: {case}")
        for i, row in enumerate(rows, 1):
            if int(row["frame"]) != i:
                raise ValueError(f"frame discontinuity in {case}: {i}")
            if not all(math.isfinite(float(v)) for k, v in row.items()
                       if k not in {"backend"}):
                raise ValueError(f"nonfinite value in {case}, frame {i}")
        timed = rows[30:] if len(rows) > 30 else rows
        def maximum(column):
            return max(float(r[column]) for r in rows)
        first_contact = next((int(r["frame"]) for r in rows
                              if int(r["solver_contacts_max"]) > 0), None)
        precontact = [r for r in rows if first_contact is None or int(r["frame"]) < first_contact]
        summary = {
            "frames": len(rows), "first_solver_contact_frame": first_contact,
            "min_driven_nondriven_gap": min(float(r["driven_nondriven_min_gap"]) for r in rows),
            "max_solver_contacts": max(int(r["solver_contacts_max"]) for r in rows),
            "max_area_rel_error": maximum("max_area_rel_error"),
            "max_exchange_mismatch": maximum("exchange_mismatch"),
            "max_fluid_speed": maximum("max_fluid_speed"),
            "max_stokes_force_law_rms": maximum("stokes_force_law_rms"),
            "max_stokes_divergence_rms": maximum("stokes_divergence_rms"),
            "max_abs_lbm_clock_error": max(abs(float(r["lbm_clock_error"])) for r in rows),
            "max_non_driven_abs_vx_before_contact":
                max((float(r["non_driven_max_abs_vx"]) for r in precontact), default=None),
            "final_non_driven_mean_vx": float(rows[-1]["non_driven_mean_vx"]),
            "timing_frames": len(timed),
        }
        for column in ["whole_ms", "fluid_ms", "rapier_ms", "diagnostic_ms"]:
            summary[column] = describe([float(r[column]) for r in timed])
        for sample in [1, 15, 30, 60, 120, 360]:
            if sample <= len(rows):
                r = rows[sample - 1]
                summary[f"frame_{sample}"] = {
                    "non_driven_mean_vx": float(r["non_driven_mean_vx"]),
                    "non_driven_max_abs_vx": float(r["non_driven_max_abs_vx"]),
                    "gap": float(r["driven_nondriven_min_gap"]),
                    "contacts": int(r["solver_contacts_max"]),
                    "max_fluid_speed": float(r["max_fluid_speed"]),
                }
        out[case.name] = summary
    if not out:
        raise ValueError(f"no frames.csv under {args.results}")
    result = args.results / "summary.json"
    result.write_text(json.dumps(out, indent=2) + "\n")
    comparisons = {}
    for n in [32, 64]:
        reference = args.results / f"stokes{n}-16" / "cells.csv"
        if not reference.exists():
            continue
        a = read(reference)
        for backend in ["lagged", "lbm", "control"]:
            candidate = args.results / f"{backend}{n}-16" / "cells.csv"
            if not candidate.exists():
                continue
            b = read(candidate)
            if len(a) != len(b) or any((x["frame"], x["cell"]) != (y["frame"], y["cell"])
                                         for x, y in zip(a, b)):
                raise ValueError(f"cell row mismatch: {reference} vs {candidate}")
            center_diff = [math.hypot(float(x["center_x"])-float(y["center_x"]),
                                      float(x["center_y"])-float(y["center_y"]))
                           for x, y in zip(a, b)]
            velocity_diff = [math.hypot(float(x["velocity_x"])-float(y["velocity_x"]),
                                        float(x["velocity_y"])-float(y["velocity_y"]))
                             for x, y in zip(a, b)]
            comparisons[f"stokes{n}-16_vs_{backend}{n}-16"] = {
                "max_cell_center_distance": max(center_diff),
                "max_cell_velocity_distance": max(velocity_diff),
                "mean_cell_center_distance": statistics.fmean(center_diff),
            }
    (args.results / "comparisons.json").write_text(json.dumps(comparisons, indent=2) + "\n")
    for case, s in out.items():
        w = s["whole_ms"]
        print(f"{case:18} whole mean/p95/max {w['mean']:.3f}/{w['p95']:.3f}/{w['max']:.3f} ms"
              f"  area {s['max_area_rel_error']:.4%} contact {s['first_solver_contact_frame']}")
    for name, comp in comparisons.items():
        print(f"{name}: max center {comp['max_cell_center_distance']:.3e},"
              f" max velocity {comp['max_cell_velocity_distance']:.3e}")
    print(f"wrote {result}")


if __name__ == "__main__":
    main()
