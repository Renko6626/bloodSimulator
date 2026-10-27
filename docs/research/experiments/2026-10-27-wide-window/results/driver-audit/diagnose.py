#!/usr/bin/env python3
"""Static coarse-small control; no new forcing model or production edits."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

import run

ROOT = Path(__file__).resolve().parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--results', type=Path, default=ROOT / 'results')
    root = parser.parse_args().results.resolve()
    out = root / 'driver-audit'
    out.mkdir(exist_ok=False)
    work = Path(tempfile.mkdtemp(prefix='blood-window-driver-audit-', dir='/tmp/sunyunbo')) / 'control'
    run.prepare(root / 'sources', work, 'base')
    path = work / 'crates/simulation/src/fluid/mod.rs'
    text = run.replace(path.read_text(), 'const N: usize = 64;', 'const N: usize = 32;')
    path.write_text(run.replace(text, 'const H: f64 = 0.25;', 'const H: f64 = 0.5;'))
    shutil.copy2(ROOT / 'driver_audit.rs', work / 'crates/simulation/examples/driver_audit.rs')
    manifest = {'control': '16x16 / 32² / h=0.5; original centers; zero timesteps',
                'sources_sha256': {name: run.sha(ROOT / name) for name in
                                  ['diagnose.py', 'driver_audit.rs', 'run.py']},
                'temporary_workspace': str(work), 'exit_code': None}
    for name in manifest['sources_sha256']:
        shutil.copy2(ROOT / name, out / name)
    command = ['cargo', 'build', '--release', '--offline', '--locked', '--manifest-path',
               str(work / 'Cargo.toml'), '-p', 'simulation', '--example', 'driver_audit',
               '--target-dir', str(run.TARGET), '-j', '4']
    manifest['build_command'] = command
    with (out / 'build.log').open('w') as log:
        subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
    binary = run.TARGET / 'release/examples/driver_audit'
    manifest['binary_sha256'] = run.sha(binary)
    command = ['flock', '/tmp/blood-stokes-feedback-bench.lock', 'taskset', '-c', '2-5', str(binary)]
    manifest['run_command'] = command
    with (out / 'run.log').open('w') as log, (out / 'stderr.log').open('w') as err:
        result = subprocess.run(command, stdout=log, stderr=err)
    manifest['exit_code'] = result.returncode
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print((out / 'run.log').read_text())
    raise SystemExit(result.returncode)


if __name__ == '__main__':
    main()
