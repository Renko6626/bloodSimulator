#!/usr/bin/env python3
"""Summarize complete relaxation runs and preserve partial negative/failed trajectories."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parent
REF64 = ROOT.parent / "2026-10-26-stokes-64-cells/reference-results/optimized-t4-r1/cells.csv"
REF16 = ROOT.parent / "2026-10-26-stokes-feedback/dynamics/results/stokes64-16/cells.csv"
OLD_LAGGED = ROOT.parent / "2026-10-26-stokes-64-cells/results/optimized-t4-r1"
TIMING = ["whole_ms", "fluid_ms", "rapier_ms", "diagnostic_ms", "point_force_update_ms"]


def read(path):
    with path.open(newline="") as stream:
        return list(csv.DictReader(stream))


def stats(values):
    values = sorted(values)
    return {"mean": statistics.fmean(values),
            "p95": values[math.ceil(0.95*len(values))-1], "max": values[-1]}


def compare_cells(candidate, reference):
    candidate = read(candidate)
    reference_path = reference
    reference = read(reference_path)
    if len(candidate) != len(reference) or any((a["frame"], a["cell"]) != (b["frame"], b["cell"])
                                               for a, b in zip(candidate, reference)):
        raise ValueError("candidate/reference cell rows have different frame or cell keys")
    center = [math.hypot(float(a["center_x"])-float(b["center_x"]),
                         float(a["center_y"])-float(b["center_y"]))
              for a, b in zip(candidate, reference)]
    velocity = [math.hypot(float(a["velocity_x"])-float(b["velocity_x"]),
                           float(a["velocity_y"])-float(b["velocity_y"]))
                for a, b in zip(candidate, reference)]
    area = [abs(float(a["area_rel_error"])-float(b["area_rel_error"]))
            for a, b in zip(candidate, reference)]
    after_release = [d for row, d in zip(candidate, center) if float(row["t"]) > 0.5]
    return {"reference": str(reference_path.relative_to(ROOT.parent)),
            "max_center_distance": max(center), "max_center_distance_in_diameters": max(center)/0.09,
            "mean_center_distance": statistics.fmean(center),
            "max_center_distance_after_release": max(after_release),
            "max_velocity_distance": max(velocity),
            "max_area_rel_error_difference": max(area),
            "note": "Center distance normalized by fixed cell diameter 0.09, not tiny reference displacement."}


def compare_negative(candidate):
    checks = {}
    for filename in ["frames.csv", "cells.csv"]:
        new = read(candidate / filename)
        old = read(OLD_LAGGED / filename)[:len(new)]
        ignored = {"backend", "omega", "fluid_ms", "rapier_ms", "diagnostic_ms", "whole_ms"}
        common = sorted(set(new[0]) & set(old[0]) - ignored) if new else []
        mismatches = [(i+1, field) for i, (a, b) in enumerate(zip(new, old))
                      for field in common if a[field] != b[field]]
        checks[filename] = {"compared_rows": len(new), "common_non_timing_fields": len(common),
                            "row_count_available": len(new) == len(old),
                            "all_common_values_identical": len(new) == len(old) and not mismatches,
                            "first_mismatch": mismatches[0] if mismatches else None}
    return checks


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--results", type=Path, default=ROOT / "results")
    args = parser.parse_args()
    manifest = json.loads((args.results / "manifest.json").read_text())
    for relative, digest in manifest["sources"].items():
        path = ROOT / relative
        if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            raise ValueError(f"measured source changed: {relative}")
    summaries = {}
    for entry in manifest["runs"]:
        folder = args.results / entry["name"]
        rows = read(folder / "frames.csv")
        for i, row in enumerate(rows, 1):
            if int(row["frame"]) != i:
                raise ValueError(f"frame discontinuity in {entry['name']} at {i}")
            if not all(math.isfinite(float(v)) for v in row.values()):
                raise ValueError(f"nonfinite written frame in {entry['name']} at {i}")
        full = entry["exit_code"] == 0 and len(rows) == entry["requested_frames"]
        item = {"completed": full, "exit_code": entry["exit_code"], "written_frames": len(rows),
                "requested_frames": entry["requested_frames"], "negative_control": entry["negative_control"],
                "last_model_time": float(rows[-1]["t"]) if rows else 0,
                "run_log": (folder / "run.log").read_text().strip()}
        if rows:
            for field in ["max_area_rel_error", "max_node_jump", "max_fluid_speed", "exchange_mismatch",
                          "stokes_force_law_rms", "stokes_force_law_max_component_abs",
                          "solver_contacts_max", "non_driven_max_abs_vx"]:
                item[f"allframe_max_{field}"] = max(float(r[field]) for r in rows)
            item["allframe_max_whole_ms"] = max(float(r["whole_ms"]) for r in rows)
            item["allframe_max_fluid_ms"] = max(float(r["fluid_ms"]) for r in rows)
            item["allframe_max_rapier_ms"] = max(float(r["rapier_ms"]) for r in rows)
            item["total_full_stokes_calls"] = sum(int(r["stokes_fluid_calls"]) for r in rows)
            item["flow_calls_by_frame"] = {"first": int(rows[0]["stokes_fluid_calls"]),
                                            "subsequent_unique": sorted({int(r["stokes_fluid_calls"]) for r in rows[1:]})}
            item["positive_exchange_power_frames"] = sum(float(r["exchange_power_mean"]) > 0 for r in rows)
            item["max_positive_exchange_power_mean"] = max(0.0, max(float(r["exchange_power_mean"]) for r in rows))
            item["first_solver_contact_frame"] = next((int(r["frame"]) for r in rows
                                                        if int(r["solver_contacts_max"]) > 0), None)
            item["min_driven_nondriven_gap"] = min(float(r["driven_nondriven_min_gap"]) for r in rows)
            item["after_release_max_force_law_rms"] = max((float(r["stokes_force_law_rms"])
                for r in rows if float(r["t"]) > 0.5), default=None)
        if full and len(rows) > 30:
            for field in TIMING:
                item[field] = stats([float(r[field]) for r in rows[30:]])
            ref = REF64 if entry["count"] == 64 else REF16
            item["self_consistent_comparison"] = compare_cells(folder / "cells.csv", ref)
        if entry["negative_control"] and rows:
            item["old_lagged_common_field_check"] = compare_negative(folder)
        summaries[entry["name"]] = item
        print(f"{entry['name']}: completed={full}, frames={len(rows)}, whole={item.get('whole_ms')}, "
              f"max law={item.get('allframe_max_stokes_force_law_rms')}")
    for omega in ["omega05", "omega03"]:
        if f"{omega}-64-r1" not in summaries or f"{omega}-64-r2" not in summaries:
            continue
        a, b = summaries[f"{omega}-64-r1"], summaries[f"{omega}-64-r2"]
        if a["completed"] and b["completed"]:
            cells_a = read(args.results / f"{omega}-64-r1/cells.csv")
            cells_b = read(args.results / f"{omega}-64-r2/cells.csv")
            a["repeat_max_center_difference"] = max(math.hypot(
                float(x["center_x"])-float(y["center_x"]),
                float(x["center_y"])-float(y["center_y"])) for x, y in zip(cells_a, cells_b))
            if len(cells_a) != len(cells_b):
                raise ValueError(f"repeat row count mismatch for {omega}")
    result = args.results / "summary.json"
    result.write_text(json.dumps(summaries, indent=2) + "\n")
    print(f"wrote {result}")


if __name__ == "__main__":
    main()
