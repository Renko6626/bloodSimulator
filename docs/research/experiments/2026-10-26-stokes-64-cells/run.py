#!/usr/bin/env python3
"""64-cell probe, preserving failed runs and never timing partial runs as success."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parent
PREVIOUS = ROOT.parent / "2026-10-26-stokes-hotpaths"
TARGET = Path("/tmp/sunyunbo/blood-stokes-rayon-target")
BINS = Path("/tmp/sunyunbo/stokes-64-cells-binaries")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--reference", action="store_true", help="Only the fully self-consistent four-thread control")
    parser.add_argument("--frames", type=int, default=360)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    out = args.out or ROOT / ("reference-results" if args.reference else "results")
    out.mkdir(parents=True, exist_ok=True)
    BINS.mkdir(parents=True, exist_ok=True)
    variants = ("optimized",) if args.reference else ("baseline", "optimized")
    for variant in variants:
        command = ["cargo", "build", "--release", "--offline", "--locked", "--manifest-path", str(ROOT / "dynamics/Cargo.toml"), "--bin", "stokes-64-cells-dynamics"]
        if variant == "optimized":
            command += ["--features", "optimized"]
        subprocess.run(command, env=dict(os.environ, CARGO_TARGET_DIR=str(TARGET)), check=True)
        shutil.copy2(TARGET / "release/stokes-64-cells-dynamics", BINS / variant)
    sources = [ROOT / "dynamics/src/main.rs", ROOT / "dynamics/Cargo.toml", ROOT / "dynamics/Cargo.lock"]
    for name in ("baseline", "solver"):
        sources += list((PREVIOUS / name).glob("*.rs"))
        sources += [PREVIOUS / name / "Cargo.toml", PREVIOUS / name / "Cargo.lock"]
    sources += [ROOT.parent / "2026-10-23-lbm-many/shared/fluid.rs"]
    topology = []
    for cpu in range(2, 6):
        p = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology")
        topology.append({"cpu": cpu, "core": int((p / "core_id").read_text()), "socket": int((p / "physical_package_id").read_text())})
    assert len({(x["socket"], x["core"]) for x in topology}) == 4
    assert len({x["socket"] for x in topology}) == 1
    assert {2, 3, 4, 5} <= os.sched_getaffinity(0)
    manifest = {"platform": platform.platform(), "cpu_topology": topology,
                "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
                "count": 64, "n": 64, "radius": 0.045, "layout": "8x8 centers from 0.11 to 0.89 inclusive on each axis",
                "driven_cells": [27, 35], "drive_force": 0.18, "drive_until_seconds": 0.5,
                "backend": "stokes" if args.reference else "lagged", "profile": False,
                "sources": {os.path.relpath(p, ROOT): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(sources)},
                "binaries": {v: hashlib.sha256((BINS / v).read_bytes()).hexdigest() for v in variants},
                "runs": []}
    order = [("optimized", 4)] if args.reference else [("optimized", 4), ("optimized", 0), ("baseline", 4), ("baseline", 0)]
    for repeat in range(1, 2 if args.reference else 3):
        failed = False
        for variant, threads in (order if repeat == 1 else list(reversed(order))):
            name = f"{variant}-t{threads}-r{repeat}"
            folder = out / name
            folder.mkdir(exist_ok=True)
            command = ["flock", "/tmp/blood-stokes-feedback-bench.lock", "taskset", "-c", "2-5" if threads == 4 else "2",
                       str(BINS / variant), "--backend", manifest["backend"], "--n", "64", "--count", "64",
                       "--threads", str(threads), "--frames", str(args.frames), "--profile", "0", "--outdir", str(folder)]
            print(f"Running {manifest['backend']} {name}: {args.frames} frames", flush=True)
            cpu_before = resource.getrusage(resource.RUSAGE_CHILDREN)
            started = time.monotonic()
            with (folder / "run.log").open("w") as stream:
                process = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT)
            cpu_after = resource.getrusage(resource.RUSAGE_CHILDREN)
            manifest["runs"].append({"name": name, "variant": variant, "threads": threads, "repeat": repeat,
                                     "requested_frames": args.frames, "exit_code": process.returncode, "command": command,
                                     "process_wall_seconds": time.monotonic() - started,
                                     "process_cpu_seconds": cpu_after.ru_utime + cpu_after.ru_stime - cpu_before.ru_utime - cpu_before.ru_stime})
            (out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
            print((folder / "run.log").read_text().strip(), flush=True)
            failed |= process.returncode != 0
        if failed:
            print("Failure preserved; skipping repeat timings. Partial trajectories are not performance successes.")
            raise SystemExit(1)


if __name__ == "__main__":
    main()
