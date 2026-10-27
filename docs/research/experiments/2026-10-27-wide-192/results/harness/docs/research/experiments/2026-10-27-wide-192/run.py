#!/usr/bin/env python3
"""Matched 96² / 192² measurements, both with a fixed 48x48 physical extent."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]
PREV = ROOT.parent / '2026-10-27-wide-window'
SPEC = importlib.util.spec_from_file_location('wide_trial', PREV / 'run.py')
TRIAL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TRIAL)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--seconds', type=int, default=20)
    parser.add_argument('--out', type=Path, default=ROOT / 'results')
    args = parser.parse_args()
    assert args.seconds >= 10 and {2, 3, 4, 5} <= os.sched_getaffinity(0)
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    paths = [REPO / 'Cargo.toml', REPO / 'Cargo.lock',
             *sorted((REPO / 'crates').rglob('*.rs')),
             *sorted((REPO / 'crates').rglob('Cargo.toml'))]
    hashes = {str(p.relative_to(REPO)): sha(p) for p in paths}
    for p in paths:
        target = out / 'sources' / p.relative_to(REPO)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p, target)
    work = Path(tempfile.mkdtemp(prefix='blood-wide-192-', dir='/tmp/sunyunbo'))
    harness = [ROOT / 'run.py', ROOT / 'analyze.py', PREV / 'run.py',
               PREV / 'probe.rs', PREV / 'analyze.py']
    experiment = {str(p.relative_to(REPO)): sha(p) for p in harness}
    for p in harness:
        dest = out / 'harness' / p.relative_to(REPO)
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p, dest)
    manifest = {
        'seconds': args.seconds, 'cpus': [2, 3, 4, 5], 'sources_sha256': hashes,
        'experiment_sha256': experiment, 'temporary_workspaces': str(work),
        'audits': {}, 'commands': [], 'runs': [],
        'scene': '48x48;two fixed centers [-14,0],[-10,-8];72 cells;2 substeps;real waypoint input;known missing internal pump unchanged',
        'variants': {'base': {'side': 48, 'n': 96, 'h': .5},
                     'wide': {'side': 48, 'n': 192, 'h': .25}},
    }

    def save():
        (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')

    save()
    binaries = {}
    for variant in ['base', 'wide']:
        print('Preparing', manifest['variants'][variant]['n'], flush=True)
        folder = work / variant
        TRIAL.prepare(out / 'sources', folder, 'wide')
        if variant == 'wide':
            p = folder / 'crates/simulation/src/fluid/mod.rs'
            text = TRIAL.replace(p.read_text(), 'const N: usize = 96;', 'const N: usize = 192;')
            p.write_text(TRIAL.replace(text, 'const H: f64 = 0.5;', 'const H: f64 = 0.25;'))
        changed = {str(p.relative_to(folder)): sha(p) for p in folder.rglob('*.rs')
                   if str(p.relative_to(folder)) not in hashes or sha(p) != hashes[str(p.relative_to(folder))]}
        for name in changed:
            target = out / 'variants' / variant / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(folder / name, target)
        manifest['variants'][variant]['modified_sha256'] = changed
        common = ['--release', '--offline', '--locked', '--manifest-path', str(folder / 'Cargo.toml'),
                  '-p', 'simulation', '--target-dir', str(TRIAL.TARGET), '-j', '4']
        command = ['cargo', 'test', *common]
        manifest['commands'].append(command)
        with (out / f'{variant}-audit.log').open('w') as log:
            result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
        manifest['audits'][variant] = result.returncode
        save()
        if result.returncode:
            raise SystemExit(f'{variant} existing checks failed; inspect audit log')
        command = ['cargo', 'build', *common, '--example', 'window_probe']
        manifest['commands'].append(command)
        save()
        with (out / f'{variant}-build.log').open('w') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
        binaries[variant] = work / f'{variant}-probe'
        shutil.copy2(TRIAL.TARGET / 'release/examples/window_probe', binaries[variant])
        manifest['variants'][variant]['binary_sha256'] = sha(binaries[variant])
        save()
    for name, variant in [('base-r1', 'base'), ('wide-r1', 'wide'),
                          ('wide-r2', 'wide'), ('base-r2', 'base')]:
        folder = out / name
        folder.mkdir()
        command = ['flock', '/tmp/blood-stokes-feedback-bench.lock', 'taskset', '-c', '2-5',
                   '/usr/bin/time', '-f', 'MAX_RSS_KIB,%M', str(binaries[variant]),
                   str(args.seconds), str(folder / 'drive')]
        print('Running', name, 'n=', manifest['variants'][variant]['n'], flush=True)
        with (folder / 'run.log').open('w') as log, (folder / 'stderr.log').open('w') as err:
            result = subprocess.run(command, stdout=log, stderr=err)
        manifest['runs'].append({'name': name, 'variant': variant, 'command': command,
                                 'exit_code': result.returncode})
        save()
        if result.returncode:
            raise SystemExit(f'{name} failed; raw data retained')
    environment = {'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
                   'cpu': subprocess.check_output(['lscpu'], text=True)}
    (out / 'environment.json').write_text(json.dumps(environment, indent=2) + '\n')
    assert all(sha(REPO / name) == expected for name, expected in hashes.items())
    print('Completed', out, flush=True)


if __name__ == '__main__':
    main()
