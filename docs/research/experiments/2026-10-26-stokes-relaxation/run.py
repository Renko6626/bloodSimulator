#!/usr/bin/env python3
"""Run the approved finite-drag relaxation spike with preserved failures."""
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
CASES = [
    ("omega05-16", 16, 0.5, 360, False),
    ("omega05-64-r1", 64, 0.5, 360, False),
    ("omega03-64-r1", 64, 0.3, 360, False),
    ("omega03-64-r2", 64, 0.3, 360, False),
    ("omega05-64-r2", 64, 0.5, 360, False),
    ("omega1-64-negative", 64, 1.0, 32, True),
]


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    paths = [ROOT / "dynamics/Cargo.toml", ROOT / "dynamics/Cargo.lock",
             ROOT / "dynamics/src/main.rs"]
    paths += sorted((ROOT / "solver").glob("*.rs"))
    paths += [ROOT / "solver/Cargo.toml", ROOT / "solver/Cargo.lock"]
    return {str(p.relative_to(ROOT)): sha256(p) for p in paths}


def topology():
    cpus = []
    for cpu in range(2, 6):
        folder = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology")
        cpus.append({"cpu": cpu, "core": int((folder / "core_id").read_text()),
                     "socket": int((folder / "physical_package_id").read_text())})
    assert len({(c["socket"], c["core"]) for c in cpus}) == 4
    assert len({c["socket"] for c in cpus}) == 1
    assert {2, 3, 4, 5} <= os.sched_getaffinity(0)
    return cpus


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--case", choices=[c[0] for c in CASES], action="append")
    parser.add_argument("--frames", type=int, help="Override frame count for a scratch run")
    parser.add_argument("--out", type=Path, default=ROOT / "results")
    args = parser.parse_args()
    if args.frames is not None and args.frames <= 0:
        parser.error("--frames must be positive")
    args.out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET))
    subprocess.run(["cargo", "build", "--release", "--offline", "--locked", "--manifest-path",
                    str(ROOT / "dynamics/Cargo.toml")], check=True, env=env)
    binary = TARGET / "release/stokes-relaxation-dynamics"
    source_hashes = sources()
    manifest = {
        "sources": source_hashes, "binary_sha256": sha256(binary),
        "platform": platform.platform(),
        "cpu_model": next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines()
                           if line.startswith("model name")), "unknown"),
        "cpu_topology": topology(),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "n": 64, "threads": 4, "radius": 0.045, "contact_particle_radius": 0.0045,
        "mass_per_cell": 1.0, "drag_total_per_cell": 0.25,
        "drive_force_per_driven_cell": 0.18, "drive_until_seconds": 0.5,
        "frame_hz": 60, "rapier_substeps": 2,
        "timing": "release; flock shared lock; taskset CPU 2-5; first 30 frames excluded from summary",
        "runs": [],
    }
    failed_positive = False
    for name, count, omega, default_frames, negative in CASES:
        if args.case and name not in args.case:
            continue
        frames = args.frames or default_frames
        folder = args.out / name
        folder.mkdir(parents=True, exist_ok=True)
        command = ["flock", "/tmp/blood-stokes-feedback-bench.lock", "taskset", "-c", "2-5",
                   str(binary), "--n", "64", "--count", str(count), "--threads", "4",
                   "--omega", str(omega), "--frames", str(frames), "--outdir", str(folder)]
        print(f"Running {name}: {frames} frames", flush=True)
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        started = time.monotonic()
        with (folder / "run.log").open("w") as stream:
            process = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT)
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        entry = {"name": name, "count": count, "omega": omega, "negative_control": negative,
                 "requested_frames": frames, "exit_code": process.returncode,
                 "command": command, "process_wall_seconds": time.monotonic()-started,
                 "process_cpu_seconds": after.ru_utime+after.ru_stime-before.ru_utime-before.ru_stime}
        manifest["runs"].append(entry)
        (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print((folder / "run.log").read_text().strip(), flush=True)
        if process.returncode and not negative:
            failed_positive = True
    if sources() != source_hashes:
        raise SystemExit("Source changed during run; manifest preserves original hashes")
    if failed_positive:
        raise SystemExit("One or more approved relaxation cases failed; partial data preserved")


if __name__ == "__main__":
    main()
