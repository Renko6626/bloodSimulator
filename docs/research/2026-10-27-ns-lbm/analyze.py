#!/usr/bin/env python3
"""Summarize archived per-frame measurements; p95 uses nearest-rank samples."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import statistics


def p95(xs):
    return sorted(xs)[math.ceil(0.95 * len(xs)) - 1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("results", type=Path)
    args = parser.parse_args()
    out = args.results.resolve()
    manifest = json.loads((out / "manifest.json").read_text())
    if manifest["status"] != "complete":
        raise RuntimeError("incomplete run: retain raw records; do not report as completed")
    groups = {}
    for path in sorted(out.glob("r*-*x*-c*.json")):
        meta = json.loads(path.read_text())
        with path.with_suffix(".csv").open() as f:
            rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(f)]
        assert len(rows) == meta["frames"]
        assert all(math.isfinite(v) for row in rows for v in row.values())
        key = (meta["nx"], meta["ny"], meta["cells"])
        groups.setdefault(key, []).append((path.stem, meta, rows))
    assert len(groups) == 8 and all(len(runs) == 2 for runs in groups.values())
    summary = []
    report = ["# CPU NS 首批实验原始汇总", "", f"数据目录：`{out.name}`。每配置两轮反序，各丢弃 30 帧热身后记录 {manifest['frames']} 帧。p95 使用 nearest-rank，按整帧观测值直接计算。", "", "| 活跃网格 | 细胞 | 流体+耦合 mean / p95 ms | 完整 mean / p95 ms | 两轮完整 p95 ms |", "| --- | ---: | ---: | ---: | ---: |"]
    for key, runs in sorted(groups.items()):
        all_rows = [r for _, _, rows in runs for r in rows]
        metrics = {}
        for field in ("advection_ms", "diffusion_ms", "pressure_ms", "coupling_ms", "rapier_ms", "diagnostics_ms", "fluid_coupling_ms", "whole_ms"):
            xs = [r[field] for r in all_rows]
            metrics[field] = {"mean": statistics.mean(xs), "p95": p95(xs), "max": max(xs)}
        peaks = {field: max(r[field] for r in all_rows) for field in ("divergence_rms", "max_area_rel", "max_node_speed", "exchange_mismatch")}
        last = [{field: rows[-1][field] for field in ("mean_u", "cell_mean_vx", "max_area_rel")} for _, _, rows in runs]
        round_p95 = [p95([r["whole_ms"] for r in rows]) for _, _, rows in runs]
        entry = {"nx": key[0], "ny": key[1], "cells": key[2], "samples": len(all_rows), "timing": metrics, "peaks": peaks,
                 "round_names": [name for name, _, _ in runs], "round_whole_p95": round_p95, "final": last,
                 "initialization_ms": [{"fluid": m["fluid_init_ms"], "cells": m["cell_init_ms"]} for _, m, _ in runs],
                 "numeric_factor_bytes": runs[0][1]["numeric_factor_bytes"], "max_rss": [m["max_rss_kib"] for _, m, _ in runs]}
        summary.append(entry)
        f = metrics["fluid_coupling_ms"]; w = metrics["whole_ms"]
        report.append(f"| {key[0]}×{key[1]} | {key[2]} | {f['mean']:.3f} / {f['p95']:.3f} | {w['mean']:.3f} / {w['p95']:.3f} | {round_p95[0]:.3f} / {round_p95[1]:.3f} |")
    report += ["", "| 活跃网格 | 细胞 | 对流 mean ms | 黏性 mean ms | 投影 mean ms | 耦合 mean ms | Rapier mean ms | 诊断 mean ms |", "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for entry in summary:
        timings = entry["timing"]
        report.append(f"| {entry['nx']}×{entry['ny']} | {entry['cells']} | " + " | ".join(f"{timings[field]['mean']:.3f}" for field in ("advection_ms", "diffusion_ms", "pressure_ms", "coupling_ms", "rapier_ms", "diagnostics_ms")) + " |")
    physics = json.loads((out / "physics/physics.json").read_text())
    report += ["", "| 物理场景 | 网格 | Re | dt | 相对 L2 | 全时段最大 L2 | RMS 散度 |", "| --- | --- | ---: | ---: | ---: | ---: | ---: |"]
    for case in physics["cases"]:
        e = case.get("relative_l2", case.get("fluctuation_relative_l2"))
        report.append(f"| {case['case']} | {case['nx']}×{case['ny']} | {case.get('re', '—')} | {case['dt']:.6f} | {e:.6%} | {case.get('max_relative_l2', e):.6%} | {case['divergence_rms']:.2e} |")
    source = Path(__file__)
    result = {"runs": summary, "physics": physics, "analyzer_sha256": hashlib.sha256(source.read_bytes()).hexdigest()}
    (out / "summary.json").write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")
    (out / "summary.md").write_text("\n".join(report) + "\n")
    (out / "analyze.py").write_bytes(source.read_bytes())
    print("\n".join(report))


if __name__ == "__main__":
    main()
