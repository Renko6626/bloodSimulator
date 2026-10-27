#!/usr/bin/env python3
"""Compare contact prediction and Rapier parallelism in the existing coupled scene."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import time

ROOT = Path(__file__).resolve().parent
TARGET = Path("/tmp/sunyunbo/blood-stokes-rayon-target")
# The four primary configurations run twice, with their second pass reversed.
# One- and two-thread parallel builds are small scheduling-cost comparisons.
CASES = [
    ("serial-original-r1", False, 1, 0.02),
    ("serial-tuned-r1", False, 1, 0.002),
    ("parallel-original-t4-r1", True, 4, 0.02),
    ("parallel-tuned-t4-r1", True, 4, 0.002),
    ("parallel-tuned-t2", True, 2, 0.002),
    ("parallel-tuned-t1", True, 1, 0.002),
    ("parallel-tuned-t4-r2", True, 4, 0.002),
    ("parallel-original-t4-r2", True, 4, 0.02),
    ("serial-tuned-r2", False, 1, 0.002),
    ("serial-original-r2", False, 1, 0.02),
]


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    paths = [ROOT / "dynamics/Cargo.toml", ROOT / "dynamics/Cargo.lock",
             ROOT / "dynamics/src/main.rs", ROOT / "run.py"]
    solver = ROOT.parent / "2026-10-26-stokes-relaxation/solver"
    paths += sorted(solver.glob("*.rs"))
    paths += [solver / "Cargo.toml", solver / "Cargo.lock"]
    return {os.path.relpath(p, ROOT): sha256(p) for p in paths}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--case", choices=[c[0] for c in CASES], action="append")
    parser.add_argument("--frames", type=int, default=360)
    parser.add_argument("--out", type=Path, default=ROOT / "results")
    args = parser.parse_args()
    if args.frames <= 0:
        parser.error("frames must be positive")
    args.out.mkdir(parents=True, exist_ok=True)
    topology = []
    for cpu in range(2, 6):
        path = Path(f"/sys/devices/system/cpu/cpu{cpu}/topology")
        topology.append({"cpu": cpu, "core": int((path / "core_id").read_text()),
                         "socket": int((path / "physical_package_id").read_text())})
    assert {2, 3, 4, 5} <= os.sched_getaffinity(0)
    assert len({(c["socket"], c["core"]) for c in topology}) == 4
    assert len({c["socket"] for c in topology}) == 1

    binaries = {}
    for parallel in (False, True):
        command = ["cargo", "build", "--release", "--offline", "--locked",
                   "--manifest-path", str(ROOT / "dynamics/Cargo.toml"),
                   "--target-dir", str(TARGET)]
        if not parallel:
            command.append("--no-default-features")
        subprocess.run(command, check=True)
        binary = TARGET / ("rapier-parallel-on" if parallel else "rapier-parallel-off")
        shutil.copy2(TARGET / "release/rapier-parallel-dynamics", binary)
        binaries[parallel] = binary

    source_hashes = sources()
    manifest = {
        "sources": source_hashes,
        "binary_sha256": {str(k): sha256(v) for k, v in binaries.items()},
        "platform": platform.platform(), "cpu_topology": topology,
        "cpu_model": next(line.split(":", 1)[1].strip()
                          for line in Path("/proc/cpuinfo").read_text().splitlines()
                          if line.startswith("model name")),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "scene": "64^2; 64 cells; radius .045; 12 nodes; 4 drag markers; omega .5; fluid 4 threads",
        "timing": "release; no internal profiler; CPU 2-5; shared lock; exclude first 30 frames",
        "runs": [],
    }
    failed = False
    for name, parallel, threads, prediction in CASES:
        if args.case and name not in args.case:
            continue
        folder = args.out / name
        folder.mkdir(parents=True, exist_ok=True)
        command = ["flock", "/tmp/blood-stokes-feedback-bench.lock", "taskset", "-c", "2-5",
                   str(binaries[parallel]), "--n", "64", "--count", "64", "--threads", "4",
                   "--omega", "0.5", "--frames", str(args.frames), "--outdir", str(folder),
                   "--prediction", str(prediction), "--rapier-threads", str(threads)]
        print(f"Running {name}", flush=True)
        started = time.monotonic()
        with (folder / "run.log").open("w") as stream:
            process = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT)
        manifest["runs"].append({"name": name, "parallel": parallel, "threads": threads,
                                 "prediction": prediction, "requested_frames": args.frames,
                                 "exit_code": process.returncode, "command": command,
                                 "process_wall_seconds": time.monotonic() - started})
        (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print((folder / "run.log").read_text().strip(), flush=True)
        failed |= process.returncode != 0
    if sources() != source_hashes:
        raise SystemExit("Source changed during timing; original hashes preserved")
    if failed:
        raise SystemExit("At least one case failed; partial data preserved")


if __name__ == "__main__":
    main()
