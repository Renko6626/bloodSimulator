#!/usr/bin/env python3
"""Reproduce the bounded two-region driving probe on production simulation."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]
TARGET = Path('/tmp/sunyunbo/blood-stokes-rayon-target')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--seconds', type=float, default=20)
    parser.add_argument('--out', type=Path, default=ROOT / 'results')
    args = parser.parse_args()
    assert args.seconds > 0 and {2, 3, 4, 5} <= os.sched_getaffinity(0)
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    paths = [REPO / 'Cargo.toml', REPO / 'Cargo.lock', REPO / 'crates/simulation/Cargo.toml',
             REPO / 'crates/godot_bridge/Cargo.toml', REPO / 'godot/world.gd', ROOT / 'run.py',
             *sorted((REPO / 'crates').rglob('*.rs'))]
    sources = {str(p.relative_to(REPO)): sha(p) for p in paths}
    for p in paths:
        target = out / 'sources' / p.relative_to(REPO)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p, target)
    manifest = {'seconds': args.seconds, 'cpus': [2, 3, 4, 5], 'sources_sha256': sources,
                'scene': 'production joint LU;two precomputed regions;6 samples;72 cells;real MoveInput waypoint driving',
                'commands': [], 'exit_code': None}
    command = ['cargo', 'build', '--release', '--offline', '--locked', '-p', 'simulation',
               '--example', 'region_probe', '--target-dir', str(TARGET), '-j', '4']
    manifest['commands'].append(command)
    with (out / 'build.log').open('w') as log:
        subprocess.run(command, cwd=REPO, stdout=log, stderr=subprocess.STDOUT, check=True)
    binary = TARGET / 'release/examples/region_probe'
    manifest['binary_sha256'] = sha(binary)
    command = ['flock', '/tmp/blood-stokes-feedback-bench.lock', 'taskset', '-c', '2-5',
               '/usr/bin/time', '-f', 'MAX_RSS_KIB,%M', str(binary), str(args.seconds), str(out / 'drive')]
    manifest['commands'].append(command)
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    with (out / 'run.log').open('w') as log, (out / 'stderr.log').open('w') as err:
        result = subprocess.run(command, cwd=REPO, stdout=log, stderr=err)
    manifest['exit_code'] = result.returncode
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    environment = {'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
                   'cpu': subprocess.check_output(['lscpu'], text=True)}
    (out / 'environment.json').write_text(json.dumps(environment, indent=2) + '\n')
    assert all(sha(REPO / p) == h for p, h in sources.items())
    raise SystemExit(result.returncode)


if __name__ == '__main__':
    main()
