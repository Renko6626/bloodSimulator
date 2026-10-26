#!/usr/bin/env python3
"""Run matched serial/Rayon trajectories in forward and reverse thread order."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import subprocess
import time

ROOT = Path(__file__).resolve().parent
TARGET = Path("/tmp/sunyunbo/blood-stokes-rayon-target")
LOCK = "/tmp/blood-stokes-feedback-bench.lock"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--out", type=Path, default=ROOT / "results")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET))
    subprocess.run(["cargo", "build", "--release", "--offline", "--locked", "--manifest-path",
                    str(ROOT / "dynamics/Cargo.toml")], check=True, env=env)
    cpu_info = []
    for cpu in (2, 3, 4, 5):
        path = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology")
        cpu_info.append({"cpu": cpu, "core": int((path / "core_id").read_text()),
                         "socket": int((path / "physical_package_id").read_text())})
    assert len({(x["socket"], x["core"]) for x in cpu_info}) == 4
    assert len({x["socket"] for x in cpu_info}) == 1
    assert {2, 3, 4, 5} <= os.sched_getaffinity(0)
    source_paths = list((ROOT / "solver").glob("*.rs")) + [
        ROOT / "solver/Cargo.toml", ROOT / "solver/Cargo.lock",
        ROOT / "dynamics/src/main.rs", ROOT / "dynamics/Cargo.toml", ROOT / "dynamics/Cargo.lock",
        ROOT.parent / "2026-10-23-lbm-many/shared/fluid.rs",
    ]
    manifest = {
        "platform": platform.platform(), "cpu_topology": cpu_info,
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "mode": "lagged", "count": 16, "frame_hz": 60, "substeps": 2,
        "warmup_frames": 30, "smoke": args.smoke,
        "sources": {os.path.relpath(p, ROOT): hashlib.sha256(p.read_bytes()).hexdigest()
                    for p in sorted(source_paths)},
        "note": "Non-exclusive host. Serial and Rayon1 use CPU2; Rayon2 CPU2,3; Rayon4 CPU2,3,4,5. "
                "Pool initialization is outside frame timing. Threads=0 retains original serial layout. "
                "ThreadPool workers inherit process affinity; workers are not individually pinned.",
        "runs": [],
    }
    for n in ([64] if args.smoke else [64, 128]):
        frames = 2 if args.smoke else (360 if n == 64 else 120)
        for repeat, order in enumerate(([0, 1, 2, 4],) if args.smoke else ([0, 1, 2, 4], [4, 2, 1, 0]), 1):
            for threads in order:
                name = f"n{n}-t{threads}-r{repeat}"
                folder = args.out / name
                folder.mkdir(exist_ok=True)
                cpus = ",".join(str(i) for i in range(2, 2 + max(1, threads)))
                command = ["flock", LOCK, "taskset", "-c", cpus,
                           str(TARGET / "release/stokes-rayon-dynamics"),
                           "--backend", "lagged", "--n", str(n), "--count", "16",
                           "--threads", str(threads), "--frames", str(frames), "--outdir", str(folder)]
                print(f"Running {name}: {frames} frames; CPU {cpus}", flush=True)
                before = resource.getrusage(resource.RUSAGE_CHILDREN)
                start = time.monotonic()
                with (folder / "run.log").open("w") as stream:
                    result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT)
                after = resource.getrusage(resource.RUSAGE_CHILDREN)
                entry = {"name": name, "n": n, "threads": threads, "repeat": repeat,
                         "frames": frames, "cpus": cpus, "command": command,
                         "exit_code": result.returncode, "process_wall_seconds": time.monotonic() - start,
                         "process_cpu_seconds": after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime}
                manifest["runs"].append(entry)
                (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
                if result.returncode:
                    raise RuntimeError(f"{name} failed; see {folder / 'run.log'}")
                print((folder / "run.log").read_text().strip(), flush=True)


if __name__ == "__main__":
    main()
