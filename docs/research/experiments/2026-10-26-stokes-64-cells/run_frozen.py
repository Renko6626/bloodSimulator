#!/usr/bin/env python3
"""Isolate the lagged feedback map from all mechanical/contact updates."""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent
TARGET = Path("/tmp/sunyunbo/blood-stokes-rayon-target")
out = ROOT / "frozen-results"
out.mkdir(exist_ok=True)
subprocess.run(["cargo", "build", "--release", "--offline", "--locked", "--manifest-path",
                str(ROOT / "dynamics/Cargo.toml"), "--target-dir", str(TARGET), "--bin", "frozen_feedback"], check=True)
inputs = {
    16: ROOT.parent / "2026-10-26-stokes-hotpaths/results/optimized-n64-t4-r1/outlines.csv",
    64: ROOT / "results/optimized-t4-r1/outlines.csv",
}
manifest = {"note": "Frozen frame-1 geometry; all marker velocities zero; background force removed after seeding. Not a timing benchmark.",
            "source_sha256": hashlib.sha256((ROOT / "dynamics/src/bin/frozen_feedback.rs").read_bytes()).hexdigest(), "runs": []}
for count, path in inputs.items():
    command = ["flock", "/tmp/blood-stokes-feedback-bench.lock", "taskset", "-c", "2-5",
               str(TARGET / "release/frozen_feedback"), str(path)]
    with (out / f"cells-{count}.csv").open("w") as stream:
        subprocess.run(command, stdout=stream, check=True)
    manifest["runs"].append({"count": count, "command": command, "input_sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
(out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(out)
