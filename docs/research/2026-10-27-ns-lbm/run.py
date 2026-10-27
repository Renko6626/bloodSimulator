#!/usr/bin/env python3
"""Offline release build, analytic checks, two reverse-order benchmark rounds."""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tarfile
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
LAB = HERE / "lab"
TARGET = ROOT / "target/ns-lab"
BINARY = TARGET / "release/ns-mac-lab"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def capture(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--frames", type=int, default=240)
    args = parser.parse_args()
    if args.frames < 1:
        parser.error("--frames must be positive")
    out = args.out or HERE / "results" / dt.datetime.now().strftime("local-%Y%m%d-%H%M%S")
    out = out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ, CARGO_TARGET_DIR=str(TARGET), RAYON_NUM_THREADS="4")
    commands = []
    manifest = {
        "started": dt.datetime.now(dt.timezone.utc).isoformat(),
        "host": platform.node(), "platform": platform.platform(),
        "cpu": capture("lscpu"), "rustc": capture("rustc", "--version"),
        "git_head": capture("git", "rev-parse", "HEAD"),
        "git_status": capture("git", "status", "--short"),
        "caller_affinity": sorted(os.sched_getaffinity(0)), "requested_affinity": [2, 3, 4, 5],
        "rayon_threads": 4, "faer_parallelism": "Seq",
        "loadavg_start": Path("/proc/loadavg").read_text().strip(),
        "commands": commands, "frames": args.frames, "status": "running",
    }
    if not {2, 3, 4, 5} <= os.sched_getaffinity(0):
        raise RuntimeError("requested CPUs 2-5 are outside current affinity")

    def save():
        (out / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")

    def run(cmd, name, pinned=False):
        actual = (["taskset", "-c", "2-5"] if pinned else []) + list(map(str, cmd))
        start = time.monotonic()
        entry = {"argv": actual, "cwd": str(ROOT), "log": name + ".log"}
        commands.append(entry)
        save()
        print("running", name, flush=True)
        with (out / (name + ".log")).open("w") as log:
            result = subprocess.run(actual, cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT)
        entry.update(returncode=result.returncode, wall_seconds=time.monotonic() - start)
        save()
        if result.returncode:
            raise RuntimeError(f"{name} failed; see {out / (name + '.log')}")

    save()
    try:
        run(["cargo", "build", "--release", "--offline", "--locked", "--manifest-path", LAB / "Cargo.toml"], "build")
        sources = sorted((LAB / "src").glob("*.rs")) + [LAB / "Cargo.toml", LAB / "Cargo.lock", HERE / "run.py"]
        manifest["sources_sha256"] = {str(p.relative_to(ROOT)): sha(p) for p in sources}
        manifest["binary_sha256"] = sha(BINARY)
        with tarfile.open(out / "sources.tar.gz", "w:gz") as archive:
            for p in sources:
                archive.add(p, arcname=str(p.relative_to(HERE)))
        save()
        run([BINARY, "physics", out / "physics"], "physics", pinned=True)
        configs = [(192, 192, 0), (256, 256, 0), (192, 30, 0), (256, 40, 0),
                   (192, 192, 72), (256, 256, 72), (192, 30, 72), (256, 40, 72)]
        for round_id, ordered in enumerate([configs, list(reversed(configs))], 1):
            for nx, ny, cells in ordered:
                name = f"r{round_id}-{nx}x{ny}-c{cells}"
                run([BINARY, "bench", nx, ny, cells, args.frames, out / name], name, pinned=True)
        manifest["status"] = "complete"
    except BaseException as exc:
        manifest["status"] = "failed"
        manifest["error"] = repr(exc)
        raise
    finally:
        manifest["finished"] = dt.datetime.now(dt.timezone.utc).isoformat()
        manifest["loadavg_end"] = Path("/proc/loadavg").read_text().strip()
        save()
    print(out, flush=True)


if __name__ == "__main__":
    main()
