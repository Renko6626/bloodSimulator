#!/usr/bin/env python3
"""Matched cold-pressure, warm-pressure and local-kernel comparisons."""
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
TARGET = Path('/tmp/sunyunbo/blood-stokes-rayon-target')
FEATURES = {
    'baseline': 'parallel',
    'warm': 'parallel,warm-pressure',
    'optimized': 'parallel,warm-pressure,reciprocal,fft-slices',
}
CASES = [
    ('baseline-r1', 'baseline', 360, False),
    ('warm-r1', 'warm', 360, False),
    ('optimized-r1', 'optimized', 360, False),
    ('optimized-r2', 'optimized', 360, False),
    ('warm-r2', 'warm', 360, False),
    ('baseline-r2', 'baseline', 360, False),
    ('profile-baseline', 'baseline', 120, True),
    ('profile-optimized', 'optimized', 120, True),
]


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    paths = [ROOT/'dynamics/Cargo.toml', ROOT/'dynamics/Cargo.lock',
             ROOT/'dynamics/src/main.rs', ROOT/'run.py', ROOT/'solver/Cargo.toml',
             ROOT/'solver/Cargo.lock'] + sorted((ROOT/'solver').glob('*.rs'))
    return {str(p.relative_to(ROOT)): sha256(p) for p in paths}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--case', choices=[c[0] for c in CASES], action='append')
    parser.add_argument('--frames', type=int)
    parser.add_argument('--out', type=Path, default=ROOT/'results')
    args = parser.parse_args()
    if args.frames is not None and args.frames <= 0:
        parser.error('frames must be positive')
    args.out.mkdir(parents=True, exist_ok=True)
    topology = []
    for cpu in range(2, 6):
        path = Path(f'/sys/devices/system/cpu/cpu{cpu}/topology')
        topology.append({'cpu': cpu, 'core': int((path/'core_id').read_text()),
                         'socket': int((path/'physical_package_id').read_text())})
    assert {2, 3, 4, 5} <= os.sched_getaffinity(0)
    assert len({(c['socket'], c['core']) for c in topology}) == 4
    assert len({c['socket'] for c in topology}) == 1
    binaries = {}
    for name, features in FEATURES.items():
        subprocess.run(['cargo', 'build', '--release', '--offline', '--locked',
                        '--manifest-path', str(ROOT/'dynamics/Cargo.toml'),
                        '--target-dir', str(TARGET), '--no-default-features',
                        '--features', features], check=True)
        binary = TARGET/f'stokes-kernels-{name}'
        shutil.copy2(TARGET/'release/stokes-kernel-dynamics', binary)
        binaries[name] = binary
    before = sources()
    manifest = {
        'sources': before, 'features': FEATURES,
        'binary_sha256': {name: sha256(p) for name, p in binaries.items()},
        'platform': platform.platform(), 'cpu_topology': topology,
        'cpu_model': next(line.split(':', 1)[1].strip() for line in
                          Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),
        'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
        'scene': '64^2; 64 cells; omega .5; prediction .002; fluid and Rapier 4 threads each',
        'timing': 'release; CPU 2-5; shared lock; discard first 30 frames; profile cases separate',
        'runs': [],
    }
    failed = False
    for name, variant, frames, profiling in CASES:
        if args.case and name not in args.case:
            continue
        frames = args.frames or frames
        folder = args.out/name
        folder.mkdir(parents=True, exist_ok=True)
        env = dict(os.environ)
        env.pop('STOKES_PROFILE', None)
        if profiling:
            env['STOKES_PROFILE'] = '1'
        command = ['flock', '/tmp/blood-stokes-feedback-bench.lock', 'taskset', '-c', '2-5',
                   str(binaries[variant]), '--count', '64', '--omega', '0.5', '--frames', str(frames),
                   '--outdir', str(folder), '--prediction', '0.002', '--rapier-threads', '4']
        print('Running', name, flush=True)
        start = time.monotonic()
        with (folder/'run.log').open('w') as stream:
            process = subprocess.run(command, env=env, stdout=stream, stderr=subprocess.STDOUT)
        manifest['runs'].append({'name': name, 'variant': variant, 'profiling': profiling,
                                 'requested_frames': frames, 'exit_code': process.returncode,
                                 'process_wall_seconds': time.monotonic()-start, 'command': command})
        (args.out/'manifest.json').write_text(json.dumps(manifest, indent=2)+'\n')
        print((folder/'run.log').read_text().strip(), flush=True)
        failed |= process.returncode != 0
    assert sources() == before, 'Sources changed during measurement'
    if failed:
        raise SystemExit('At least one case failed; partial results preserved')


if __name__ == '__main__':
    main()
