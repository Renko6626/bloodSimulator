#!/usr/bin/env python3
"""Analyze completed/failed 64-cell runs without hiding partial trajectories."""
import csv
import hashlib
import json
import math
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parent
TIMING = {"whole_ms", "fluid_ms", "rapier_ms", "diagnostic_ms"}


def read(path):
    with path.open(newline="") as stream:
        return list(csv.DictReader(stream))


def stats(values):
    values = sorted(values)
    return {"mean": statistics.fmean(values), "p95": values[math.ceil(0.95 * len(values)) - 1], "max": max(values)}


def main():
    combined = {}
    for result_name in ("results", "reference-results"):
        folder = ROOT / result_name
        if not (folder / "manifest.json").exists():
            continue
        manifest = json.loads((folder / "manifest.json").read_text())
        for name, digest in manifest["sources"].items():
            assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == digest, name
        summaries, pooled = {}, {}
        reference_rows = None
        reference_folder = None
        for entry in manifest["runs"]:
            name = entry["name"]
            rows = read(folder / name / "frames.csv")
            for i, row in enumerate(rows, 1):
                assert int(row["frame"]) == i
                assert all(math.isfinite(float(v)) for k, v in row.items() if k != "backend")
            if reference_rows is None:
                reference_rows, reference_folder = rows, folder / name
            assert len(rows) == len(reference_rows), (name, "different completion length")
            for a, b in zip(reference_rows, rows):
                assert {k: v for k, v in a.items() if k not in TIMING} == {k: v for k, v in b.items() if k not in TIMING}, (name, b["frame"])
            for filename in ("cells.csv", "outlines.csv", "fields.csv"):
                assert (reference_folder / filename).read_bytes() == (folder / name / filename).read_bytes(), (name, filename)
            completed = entry["exit_code"] == 0 and len(rows) == entry["requested_frames"]
            item = {"completed": completed, "exit_code": entry["exit_code"], "written_frames": len(rows),
                    "last_model_time": float(rows[-1]["t"]) if rows else 0,
                    "log": (folder / name / "run.log").read_text().strip(),
                    "first_solver_contact_frame": next((int(r["frame"]) for r in rows if int(r["solver_contacts_max"]) > 0), None)}
            for field in ("max_area_rel_error", "max_node_jump", "max_fluid_speed", "exchange_mismatch",
                          "stokes_force_law_rms", "solver_contacts_max"):
                item[field] = max((float(r[field]) for r in rows), default=None)
            if completed and len(rows) > 30:
                for field in sorted(TIMING):
                    item[field] = stats([float(r[field]) for r in rows[30:]])
                key = f"{entry['variant']}-t{entry['threads']}"
                pooled.setdefault(key, []).extend(float(r["whole_ms"]) for r in rows[30:])
            summaries[name] = item
            print(result_name, name, "completed", completed, "frames", len(rows),
                  "whole", item.get("whole_ms"), "max fluid", item["max_fluid_speed"],
                  "max force-law", item["stokes_force_law_rms"])
        result = {"backend": manifest["backend"], "runs": summaries,
                  "pooled_successful_whole_ms": {k: stats(v) for k, v in pooled.items()},
                  "non_timing_outputs_identical_across_runs": True}
        (folder / "summary.json").write_text(json.dumps(result, indent=2) + "\n")
        combined[result_name] = result
    assert combined, "No experiment data found"
    frozen = ROOT / "frozen-results"
    if (frozen / "manifest.json").exists():
        fm = json.loads((frozen / "manifest.json").read_text())
        assert hashlib.sha256((ROOT / "dynamics/src/bin/frozen_feedback.rs").read_bytes()).hexdigest() == fm["source_sha256"]
        results = {}
        for entry in fm["runs"]:
            path = Path(entry["command"][-1])
            assert hashlib.sha256(path.read_bytes()).hexdigest() == entry["input_sha256"]
            rows = read(frozen / f"cells-{entry['count']}.csv")
            assert len(rows) == 17
            for i, row in enumerate(rows):
                assert int(row["iteration"]) == i
                assert all(math.isfinite(float(v)) for v in row.values())
                if i:
                    ratio = float(row["velocity_norm"]) / float(rows[i - 1]["velocity_norm"])
                    assert math.isclose(ratio, float(row["norm_ratio"]), rel_tol=1e-10)
                    assert float(row["mean_x_velocity"]) * float(rows[i - 1]["mean_x_velocity"]) < 0
            results[str(entry["count"])] = {"final_norm_ratio": float(rows[-1]["norm_ratio"]),
                                             "zero_velocity_self_consistent_control": "asserted exactly zero by Rust probe"}
        (frozen / "summary.json").write_text(json.dumps(results, indent=2) + "\n")
        print("Frozen feedback:", results)


if __name__ == "__main__":
    main()
