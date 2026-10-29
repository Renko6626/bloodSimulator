#!/usr/bin/env python3
"""Build and run isolated diagnostic copies; no cloud or production edits."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile

p = argparse.ArgumentParser()
p.add_argument('--scratch', type=Path, required=True)
p.add_argument('--base', default='d0dfa07')
p.add_argument('--frames', type=int, default=600)
p.add_argument('--warmup', type=int, default=120)
p.add_argument('--uncoupled', action='store_true')
a = p.parse_args()
repo = Path(subprocess.check_output(['git', 'rev-parse', '--show-toplevel'], text=True).strip())
script = Path(__file__).resolve().parent
scratch = a.scratch.resolve()
scratch.mkdir(parents=True, exist_ok=True)
base = subprocess.check_output(['git', 'rev-parse', a.base], text=True).strip()
archive = subprocess.check_output(['git', 'archive', base, 'Cargo.toml', 'Cargo.lock', 'crates'])
target = scratch / 'target'
env = dict(os.environ, CARGO_TARGET_DIR=str(target))
metadata = {'base': base, 'software_only': True, 'production_defaults_changed': False,
            'frames': a.frames, 'warmup': a.warmup, 'cases': ['hold'] if a.uncoupled else ['hold', 'move'], 'uncoupled': a.uncoupled, 'builds': {}}
for name, budget in [('baseline', '0.05'), ('candidate', '0.07')]:
    source = scratch / name
    if source.exists():
        raise SystemExit(f'Choose an empty scratch directory; {source} already exists')
    source.mkdir()
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        tar.extractall(source, filter='data')
    subprocess.run(['python3', str(script / 'instrument.py'), str(source), '--budget', budget], check=True)
    if a.uncoupled:
        subprocess.run(['python3', str(script / 'instrument-uncoupled.py'), str(source)], check=True)
    with open(scratch / f'build-{name}.log', 'w') as out:
        subprocess.run(['cargo', 'build', '--locked', '--release', '--manifest-path', str(source / 'Cargo.toml'),
                        '-p', 'simulation', '--features', 'lbm-gpu', '--example', 'lbm_gameplay_probe'],
                       env=env, stdout=out, stderr=subprocess.STDOUT, check=True)
    binary = scratch / f'{name}-probe'
    shutil.copy2(target / 'release/examples/lbm_gameplay_probe', binary)
    relative = ['crates/simulation/src/fluid/lbm_coupling.rs', 'crates/simulation/src/runtime.rs',
                'crates/simulation/examples/lbm_gameplay_probe.rs']
    metadata['builds'][name] = {'budget': float(budget), 'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'source_sha256': {path: hashlib.sha256((source / path).read_bytes()).hexdigest() for path in relative}}
(scratch / 'manifest.json').write_text(json.dumps(metadata, indent=2) + '\n')
for name in ['baseline', 'candidate']:
    output = scratch / f'{name}-results'
    with open(scratch / f'{name}-run.log', 'w') as out:
        result = subprocess.run([str(scratch / f'{name}-probe'), '--frames', str(a.frames), '--warmup', str(a.warmup),
                                 '--case', 'hold' if a.uncoupled else 'both', '--allow-software', '--functional', '--output', str(output)] + (['--uncoupled'] if a.uncoupled else []),
                                stdout=out, stderr=subprocess.STDOUT)
    print(name, 'exit', result.returncode, flush=True)
    metadata['builds'][name]['run_exit'] = result.returncode
    (scratch / 'manifest.json').write_text(json.dumps(metadata, indent=2) + '\n')
    # Each probe exits on its first failure and keeps its completed CSV prefix.
