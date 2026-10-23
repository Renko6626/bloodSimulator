"""Measure the fixed-link CPU variant without replacing the baseline archive."""
import argparse
import csv
import datetime
import hashlib
import io
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--cpu', type=int, default=2)
parser.add_argument('--blocks', type=int, default=30)
args = parser.parse_args()
assert args.cpu in os.sched_getaffinity(0) and args.blocks > 1
out = ROOT / 'results' / 'optimized'
out.mkdir(exist_ok=True)
(out / 'fields').mkdir(exist_ok=True)
(out / 'blocks').mkdir(exist_ok=True)
cases = [(scene, 32, tau) for tau in (1.0, 2.0) for scene in ('straight', 'channel', 'blob4')]
cases += [('blob4', 64, 2.0)]
baseline = {row['case']: row for row in csv.DictReader((ROOT / 'results/summary.csv').open())}
metadata = dict(utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                rustc=subprocess.check_output(['rustc', '--version'], text=True).strip(),
                cpu=args.cpu, blocks=args.blocks, exclusive=False, compile_flags=['-O'],
                sha256=hashlib.sha256((ROOT / 'optimized.rs').read_bytes()).hexdigest())
(out / 'environment.json').write_text(json.dumps(metadata, indent=2))
summaries, samples = [], []
with tempfile.TemporaryDirectory(prefix='lbm-fixed-links-') as tmp:
    binary = str(Path(tmp) / 'lbm')
    subprocess.run(['rustc', '-O', str(ROOT / 'optimized.rs'), '-o', binary], check=True)
    for scene, n, tau in cases:
        steps = math.ceil((1/60)/((tau-.5)/(3*n*n)))
        case = f'{scene}_n{n}_tau{tau:g}_g0.25'
        path = out / 'blocks' / f'{case}.csv'
        field_path = out / 'fields' / f'{case}.csv'
        cmd = ['taskset', '-c', str(args.cpu), binary, '--scene', scene, '--n', str(n),
               '--tau', str(tau), '--force-amplitude', '0' if scene == 'blob4' else '.1',
               '--gamma', '.25', '--post-steps', str(steps), '--post-repeats', str(args.blocks),
               '--post-samples', str(path), '--field', str(field_path)]
        print(f'Run optimized {case}', flush=True)
        raw = subprocess.check_output(cmd, text=True)
        row = next(csv.DictReader(io.StringIO(raw)))
        for key, value in row.items():
            if key.endswith('_ms') or key.startswith('post_ms') or key == 'us_per_fluid_cell_step':
                continue
            assert value == baseline[case][key], (case, key, value, baseline[case][key])
        assert field_path.read_bytes() == (ROOT / 'results/fields' / f'{case}.csv').read_bytes(), case
        summaries.append(dict(case=case, **row))
        with path.open() as stream:
            samples.extend(dict(case=case, scene=scene, n=n, tau=tau, gamma=.25, **r)
                           for r in csv.DictReader(stream))
        print(f'  identical numerical output; block mean {row["post_ms_mean"]} ms', flush=True)
for name, rows in [('summary.csv', summaries), ('samples.csv', samples)]:
    with (out / name).open('w') as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
print('Optimized measurements and numerical-equivalence checks complete.', flush=True)
