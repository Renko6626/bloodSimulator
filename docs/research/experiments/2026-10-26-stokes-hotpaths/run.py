#!/usr/bin/env python3
"""Matched baseline/optimized runs, with profiling kept out of headline timings."""
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
TARGET = Path("/tmp/sunyunbo/blood-stokes-rayon-target")
BINARIES = Path("/tmp/sunyunbo/blood-stokes-hotpaths-binaries")
LOCK = "/tmp/blood-stokes-feedback-bench.lock"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--out", type=Path, default=ROOT / "results")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    BINARIES.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET))
    for variant in ("baseline", "optimized"):
        command = ["cargo", "build", "--release", "--offline", "--locked", "--manifest-path",
                   str(ROOT / "dynamics/Cargo.toml")]
        if variant == "optimized":
            command += ["--features", "optimized"]
        subprocess.run(command, env=env, check=True)
        shutil.copy2(TARGET / "release/stokes-hotpaths-dynamics", BINARIES / variant)
    cpu_info = []
    for cpu in (2, 3, 4, 5):
        path = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology")
        cpu_info.append({"cpu": cpu, "core": int((path / "core_id").read_text()),
                         "socket": int((path / "physical_package_id").read_text())})
    assert len({(x["socket"], x["core"]) for x in cpu_info}) == 4
    assert len({x["socket"] for x in cpu_info}) == 1
    assert {2, 3, 4, 5} <= os.sched_getaffinity(0)
    sources = []
    for directory in ("baseline", "solver"):
        sources += list((ROOT / directory).glob("*.rs"))
        sources += [ROOT / directory / "Cargo.toml", ROOT / directory / "Cargo.lock"]
    sources += [ROOT / "dynamics/src/main.rs", ROOT / "dynamics/Cargo.toml", ROOT / "dynamics/Cargo.lock",
                ROOT.parent / "2026-10-23-lbm-many/shared/fluid.rs"]
    manifest = {
        "platform": platform.platform(), "cpu_topology": cpu_info,
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "mode": "lagged", "count": 16, "frame_hz": 60, "substeps": 2,
        "warmup_frames": 30, "smoke": args.smoke,
        "sources": {os.path.relpath(p, ROOT): sha(p) for p in sorted(sources)},
        "binaries": {variant: sha(BINARIES / variant) for variant in ("baseline", "optimized")},
        "note": "Non-exclusive host; CPUs 2-5 are distinct cores on one socket. "
                "0/1 threads use CPU2, 2 threads use CPU2,3, 4 threads use CPU2-5. "
                "Headline runs disable profiling. Initialization and output excluded from frame timing.",
        "runs": [],
    }

    def run(variant, n, threads, repeat, frames, profiling=False):
        prefix = "profile-" if profiling else ""
        name = f"{prefix}{variant}-n{n}-t{threads}-r{repeat}"
        folder = args.out / name
        folder.mkdir(exist_ok=True)
        cpus = ",".join(str(cpu) for cpu in range(2, 2 + max(1, threads)))
        command = ["flock", LOCK, "taskset", "-c", cpus, str(BINARIES / variant),
                   "--backend", "lagged", "--n", str(n), "--count", "16",
                   "--threads", str(threads), "--frames", str(frames),
                   "--profile", str(int(profiling)), "--outdir", str(folder)]
        print(f"Running {name}: {frames} frames", flush=True)
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        start = time.monotonic()
        with (folder / "run.log").open("w") as stream:
            result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT)
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        manifest["runs"].append({"name": name, "variant": variant, "n": n, "threads": threads,
                                 "repeat": repeat, "frames": frames, "profiling": profiling,
                                 "command": command, "exit_code": result.returncode,
                                 "process_wall_seconds": time.monotonic() - start,
                                 "process_cpu_seconds": after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime})
        (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        if result.returncode:
            raise RuntimeError(f"{name} failed; see {folder / 'run.log'}")
        print((folder / "run.log").read_text().strip(), flush=True)

    for n in ([64] if args.smoke else [64, 128]):
        for repeat, order in enumerate(([0, 1, 2, 4],) if args.smoke else ([0, 1, 2, 4], [4, 2, 1, 0]), 1):
            for threads in order:
                variants = ("baseline", "optimized") if repeat == 1 else ("optimized", "baseline")
                for variant in variants:
                    run(variant, n, threads, repeat, 2 if args.smoke else (360 if n == 64 else 120))
    for variant in ("baseline", "optimized"):
        for threads in (0, 4):
            run(variant, 64, threads, 1, 2 if args.smoke else 120, profiling=True)


if __name__ == "__main__":
    main()
