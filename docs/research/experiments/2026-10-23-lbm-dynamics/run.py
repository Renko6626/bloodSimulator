"""Serial prescribed-marker and single-cell dynamics research, outside the game."""
import argparse
import csv
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--cpu', type=int, default=2)
parser.add_argument('--seconds', type=float, default=6)
args = parser.parse_args()
assert args.cpu in os.sched_getaffinity(0) and args.seconds >= 3
out = ROOT / 'results'
out.mkdir(exist_ok=True)
(out / 'moving').mkdir(exist_ok=True)
(out / 'rapier').mkdir(exist_ok=True)
sources = ['shared/fluid.rs', 'moving/main.rs', 'rapier/src/main.rs',
           'rapier/Cargo.toml', 'rapier/Cargo.lock']
metadata = dict(utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                rustc=subprocess.check_output(['rustc', '--version'], text=True).strip(),
                lscpu=subprocess.check_output(['lscpu'], text=True),
                cpu=args.cpu, exclusive=False, rapier_seconds=args.seconds,
                sha256={name: hashlib.sha256((ROOT/name).read_bytes()).hexdigest() for name in sources})
(out/'environment.json').write_text(json.dumps(metadata, indent=2))
with tempfile.TemporaryDirectory(prefix='lbm-dynamics-moving-') as tmp:
    binary = str(Path(tmp)/'moving')
    subprocess.run(['rustc', '--edition', '2021', '-O', '-Awarnings',
                    str(ROOT/'moving/main.rs'), '-o', binary], check=True)
    cases = [('smooth', .25, 4), ('seam', .25, 5), ('smooth', 4, 4),
             ('frozen', .25, 3190/2048)]
    for scene, gamma, seconds in cases:
        stem = f'{scene}-g{gamma:g}'
        cmd = ['taskset', '-c', str(args.cpu), binary, scene, str(gamma), str(seconds),
               str(out/'moving'/f'{stem}.csv')]
        print(f'Moving markers: {stem}, {seconds:g} model seconds', flush=True)
        result = subprocess.check_output(cmd, text=True)
        (out/'moving'/f'{stem}.log').write_text(result)
        print(result.strip(), flush=True)

manifest = ROOT/'rapier/Cargo.toml'
env = os.environ.copy()
# Reuse the project's existing build cache without changing its workspace/lockfile.
env['CARGO_TARGET_DIR'] = str(ROOT.parents[3]/'target')
subprocess.run(['cargo', 'build', '--manifest-path', str(manifest), '--offline', '--locked',
                '--release'], env=env, check=True)
binary = str(Path(env['CARGO_TARGET_DIR'])/'release/lbm-rapier-dynamics-probe')
print('Single Rapier cell: release, three scenarios', flush=True)
result = subprocess.check_output(['taskset', '-c', str(args.cpu), binary,
                                  str(out/'rapier'), str(args.seconds)], text=True)
(out/'rapier/run.log').write_text(result)
print(result.strip(), flush=True)

# Reuse the earlier frozen field at exactly 2140 settle + 30*35 post steps.
old = ROOT.parent/'2026-10-23-lbm/results/optimized/fields/blob4_n32_tau2_g0.25.csv'
new = out/'moving/frozen-g0.25.csv.field.csv'
before = list(csv.DictReader(old.open()))
after = list(csv.DictReader(new.open()))
assert len(before) == len(after) == 1024
print('Frozen field comparison will be independently checked by plot.py.', flush=True)
print('Dynamic measurements complete.', flush=True)
