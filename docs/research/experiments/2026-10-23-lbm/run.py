"""Serial LBM settling and fixed-model-time update measurements."""
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
out = ROOT / 'results'
out.mkdir(exist_ok=True)
(out / 'fields').mkdir(exist_ok=True)
(out / 'blocks').mkdir(exist_ok=True)
cases = [(scene, n, 1.0, .25) for scene in ('straight', 'channel', 'blob4') for n in (32, 64)]
cases += [(scene, 32, 2.0, .25) for scene in ('straight', 'channel', 'blob4')]
cases += [('blob4', 64, 2.0, .25), ('blob4', 32, 1.0, 4.0)]
metadata = {
    'utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
    'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
    'lscpu': subprocess.check_output(['lscpu'], text=True),
    'compile_flags': ['-O'], 'cpu': args.cpu, 'exclusive': False,
    'blocks': args.blocks, 'model_seconds_per_block_target': 1/60,
    'initial_settles_per_case': 1, 'density_reference': 1, 'viscosity': 1,
    'sha256': hashlib.sha256((ROOT / 'main.rs').read_bytes()).hexdigest(),
}
(out / 'environment.json').write_text(json.dumps(metadata, indent=2))
summaries, samples = [], []
with tempfile.TemporaryDirectory(prefix='lbm-experiment-') as tmp:
    binary = str(Path(tmp) / 'lbm')
    subprocess.run(['rustc', '-O', str(ROOT / 'main.rs'), '-o', binary], check=True)
    for scene, n, tau, gamma in cases:
        dt = (tau-.5)/(3*n*n)
        steps = math.ceil((1/60)/dt)
        case = f'{scene}_n{n}_tau{tau:g}_g{gamma:g}'
        path = out / 'blocks' / f'{case}.csv'
        cmd = ['taskset', '-c', str(args.cpu), binary, '--scene', scene, '--n', str(n),
               '--tau', str(tau), '--force-amplitude', '0' if scene == 'blob4' else '.1',
               '--gamma', str(gamma), '--post-steps', str(steps), '--post-repeats', str(args.blocks),
               '--post-samples', str(path), '--field', str(out / 'fields' / f'{case}.csv')]
        print(f'Run {case}: settle, then {args.blocks} blocks of {steps} steps', flush=True)
        raw = subprocess.check_output(cmd, text=True)
        (out / f'{case}-summary.csv').write_text(raw)
        row = next(csv.DictReader(io.StringIO(raw)))
        assert row['converged'] == 'true' and row['unstable'] == 'false', row
        assert int(row['post_steps']) == steps*args.blocks, row
        assert abs(float(row['mass_relative_error'])) < 1e-7, row
        if scene == 'blob4':
            assert abs(float(row['force_balance_abs'])) < 1e-9, row
            assert float(row['force_law_velocity_relative_rms']) < 1e-5, row
        summaries.append(dict(case=case, **row))
        with path.open() as stream:
            samples.extend(dict(case=case, scene=scene, n=n, tau=tau, gamma=gamma, **r)
                           for r in csv.DictReader(stream))
        print(f'  settle {row["settle_steps"]} steps / {row["solve_ms"]} ms; '
              f'block mean {row["post_ms_mean"]} ms', flush=True)
for name, rows in [('summary.csv', summaries), ('samples.csv', samples)]:
    with (out / name).open('w') as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
print('LBM measurements complete.', flush=True)
