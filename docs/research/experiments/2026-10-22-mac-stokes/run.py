"""Serial, CPU-pinned research measurements; no game or Cargo workspace changes."""
import argparse
import csv
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--runs", type=int, default=50)
parser.add_argument("--cpu", type=int, default=2)
args = parser.parse_args()
assert args.runs >= 2
assert args.cpu in os.sched_getaffinity(0)
out = ROOT / "results"
out.mkdir(exist_ok=True)
metadata = {
    "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
    "platform": platform.platform(),
    "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
    "compile_flags": ["-O"],
    "cpu_affinity": args.cpu,
    "runs_per_scene_size_backend": args.runs,
    "warmup_runs": 3,
    "cpu_exclusive": False,
    "sources_sha256": {
        name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
        for name in ("main.rs", "separable.rs")
    },
    "lscpu": subprocess.check_output(["lscpu"], text=True),
}
(out / "environment.json").write_text(json.dumps(metadata, indent=2))
with tempfile.TemporaryDirectory(prefix="mac-stokes-bin-") as tmp:
    binary = str(Path(tmp) / "mac_stokes")
    subprocess.run(["rustc", "-O", str(ROOT / "main.rs"), "-o", binary], check=True)
    summaries = []
    for backend in ("cg", "separable"):
        base = ["taskset", "-c", str(args.cpu), binary, "--backend", backend,
                "--scene", "all", "--sizes", "32,64,128", "--format", "csv"]
        print(f"Warmup: {backend}", flush=True)
        with (out / f"{backend}-warmup.csv").open("w") as stream:
            subprocess.run(base + ["--runs", "3"], stdout=stream, check=True)
        print(f"Measure: {backend}, {args.runs} cold solves per case", flush=True)
        path = out / f"{backend}-summary.csv"
        with path.open("w") as stream:
            subprocess.run(base + ["--runs", str(args.runs), "--samples-out",
                                   str(out / f"{backend}-samples.csv")],
                           stdout=stream, check=True)
        with path.open() as stream:
            summaries.extend(csv.DictReader(stream))
        print(f"Finished: {backend}", flush=True)
    # Separate phase-zero fields for figures; export is never inside solve timing.
    with (out / "field-export-summary.csv").open("w") as stream:
        subprocess.run(["taskset", "-c", str(args.cpu), binary, "--backend", "separable",
                        "--scene", "all", "--sizes", "64", "--runs", "1",
                        "--export-dir", str(out / "fields")], stdout=stream, check=True)
with (out / "summary.csv").open("w") as stream:
    writer = csv.DictWriter(stream, fieldnames=list(summaries[0]))
    writer.writeheader()
    writer.writerows(summaries)
print(f"Saved summaries and individual samples to {out}", flush=True)
