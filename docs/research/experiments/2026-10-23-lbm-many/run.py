"""Serial cell-count runs at fixed lattice spacing and physical timestep."""
import argparse
import csv
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--cpu', type=int, default=2)
parser.add_argument('--seconds', type=float, default=6)
args = parser.parse_args()
assert args.cpu in os.sched_getaffinity(0) and args.seconds >= 3
out = ROOT / 'results'
out.mkdir(exist_ok=True)
sources = ['shared/fluid.rs', 'rapier/src/main.rs',
           'rapier/Cargo.toml', 'rapier/Cargo.lock', 'run.py']
meta = dict(utc=datetime.datetime.now(datetime.timezone.utc).isoformat(), cpu=args.cpu,
            seconds=args.seconds, exclusive=False,
            rustc=subprocess.check_output(['rustc', '--version'], text=True).strip(),
            lscpu=subprocess.check_output(['lscpu'], text=True),
            sha256={n: hashlib.sha256((ROOT/n).read_bytes()).hexdigest() for n in sources})
env = os.environ.copy()
env['CARGO_TARGET_DIR'] = str(ROOT.parents[3] / 'target')
subprocess.run(['cargo', 'build', '--manifest-path', str(ROOT/'rapier/Cargo.toml'),
                '--offline', '--locked', '--release'], env=env, check=True)
binary = Path(env['CARGO_TARGET_DIR']) / 'release/lbm-rapier-scale-probe'
meta['binary_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
(out/'environment.json').write_text(json.dumps(meta, indent=2))
for count in (16, 32, 64, 128):
    for layout in ('expanded', 'packed'):
        dest = out / f'{layout}-{count}-cached'
        print(f'Run layout={layout}, count={count}, duration={args.seconds}', flush=True)
        result = subprocess.check_output([
            'taskset', '-c', str(args.cpu), str(binary), '--outdir', str(dest),
            '--mode', 'cached', '--scene', 'scale', '--layout', layout,
            '--count', str(count), '--duration', str(args.seconds)], text=True)
        (dest/'run.log').write_text(result)
        print(result.strip(), flush=True)

# The expanded 16-cell case preserves the previous experiment's physical inputs.
prior = ROOT.parent / '2026-10-23-lbm-scale/results/scale-16-cached'
if prior.exists() and args.seconds == 6:
    for filename in ('frames.csv', 'outlines.csv'):
        a = list(csv.DictReader((prior/filename).open()))
        b = list(csv.DictReader((out/'expanded-16-cached'/filename).open()))
        assert len(a) == len(b)
        for left, right in zip(a, b):
            for key, value in left.items():
                if not key.endswith('_ms'):
                    assert value == right[key], (filename, key, value, right[key])
    result = 'Expanded 16: all non-timing CSV fields match the previous case exactly.\n'
    (out/'baseline-equivalence.log').write_text(result)
    print(result.strip(), flush=True)
