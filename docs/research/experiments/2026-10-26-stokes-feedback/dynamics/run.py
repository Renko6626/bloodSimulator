#!/usr/bin/env python3
"""Build and run the isolated dynamic probes under the shared CPU-2 benchmark lock."""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import subprocess

HERE = pathlib.Path(__file__).resolve().parent
TARGET = pathlib.Path("/tmp/sunyunbo/blood-stokes-dynamics-release-target")
LOCK = "/tmp/blood-stokes-feedback-bench.lock"
CASES = [
    ("stokes", 32, 16), ("lagged", 32, 16), ("lbm", 32, 16),
    ("stokes", 64, 16), ("lagged", 64, 16), ("lbm", 64, 16),
    ("control", 32, 16), ("control", 32, 32), ("stokes", 32, 32),
]


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--frames", type=int, default=360)
    parser.add_argument("--case", choices=[f"{b}{n}-{count}" for b, n, count in CASES],
                        action="append", help="Run only specified case(s)")
    parser.add_argument("--out", type=pathlib.Path, default=HERE / "results")
    args = parser.parse_args()
    if args.frames <= 0:
        parser.error("--frames must be positive")
    args.out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET))
    subprocess.run(["cargo", "build", "--release", "--offline", "--locked", "--manifest-path",
                    str(HERE / "Cargo.toml")], check=True, env=env)
    binary = TARGET / "release" / "stokes-feedback-dynamics"
    selected = [(b, n, c) for b, n, c in CASES
                if args.case is None or f"{b}{n}-{c}" in args.case]
    manifests = {
        "source_sha256": sha256(HERE / "src/main.rs"),
        "measured_source_sha256": sha256(HERE / "src/main.rs"),
        "final_formatted_source_sha256": sha256(HERE / "src/main.rs"),
        "source_format_note": "No post-measurement formatting for this run.",
        "solver_sha256": sha256(HERE.parent / "solver/fluid.rs"),
        "solver_inverse_sha256": sha256(HERE.parent / "solver/separable.rs"),
        "lbm_sha256": sha256(HERE.parent.parent / "2026-10-23-lbm-many/shared/fluid.rs"),
        "lock_sha256": sha256(HERE / "Cargo.lock"),
        "python": platform.python_version(), "machine": platform.platform(),
        "frames": args.frames, "cpu": 2, "frame_hz": 60, "rapier_substeps": 2,
        "count": [16, 32], "radius": 0.045, "mass_per_cell": 1.0,
        "drag_total_per_cell": 0.25, "drive_force_per_driven_cell": 0.18,
        "drive_until_seconds": 0.5,
        "timing": "release; flock shared lock; taskset CPU 2; first 30 frames excluded in analysis",
    }
    (args.out / "manifest.json").write_text(json.dumps(manifests, indent=2) + "\n")
    for backend, n, count in selected:
        name = f"{backend}{n}-{count}"
        outdir = args.out / name
        outdir.mkdir(exist_ok=True)
        print(f"running {name}", flush=True)
        cmd = ["flock", LOCK, "taskset", "-c", "2", str(binary),
               "--backend", backend, "--n", str(n), "--count", str(count),
               "--frames", str(args.frames), "--outdir", str(outdir)]
        with (outdir / "run.log").open("w") as log:
            result = subprocess.run(cmd, stdout=log, stderr=subprocess.STDOUT)
        if result.returncode:
            raise SystemExit(f"{name} failed (exit {result.returncode}); see {outdir / 'run.log'}")
        print((outdir / "run.log").read_text().strip(), flush=True)


if __name__ == "__main__":
    main()
