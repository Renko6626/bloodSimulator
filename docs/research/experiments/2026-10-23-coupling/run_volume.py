"""Small serial repeat study; no product integration or new dependencies."""
import argparse
import csv
import datetime
import hashlib
import io
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--runs', type=int, default=20)
parser.add_argument('--cpu', type=int, default=2)
args = parser.parse_args()
assert args.runs > 1 and args.cpu in os.sched_getaffinity(0)
out = ROOT / 'results'
out.mkdir(exist_ok=True)
fields = out / 'fields'
fields.mkdir(exist_ok=True)
cases = [(scene, 32, kappa) for scene in ('channel', 'disk-static', 'disk-moving')
         for kappa in (256, 1024, 4096)]
cases += [('channel', 64, 1024), ('disk-static', 64, 1024), ('disk-moving', 64, 1024)]
metadata = dict(utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                rustc=subprocess.check_output(['rustc', '--version'], text=True).strip(),
                cpu=args.cpu, runs=args.runs, warmup_runs=1, exclusive=False,
                lscpu=subprocess.check_output(['lscpu'], text=True), compile_flags=['-O'],
                sha256={name: hashlib.sha256((ROOT / 'volume' / name).read_bytes()).hexdigest()
                        for name in ('main.rs', 'separable.rs')})
(out / 'volume-environment.json').write_text(json.dumps(metadata, indent=2))
samples, summaries = [], []
with tempfile.TemporaryDirectory(prefix='mac-volume-') as tmp:
    binary = str(Path(tmp) / 'probe')
    subprocess.run(['rustc', '-O', str(ROOT / 'volume/main.rs'), '-o', binary], check=True)
    for scene, n, kappa in cases:
        base = ['taskset', '-c', str(args.cpu), binary, '--scene', scene,
                '--n', str(n), '--kappa', str(kappa)]
        subprocess.run(base, stdout=subprocess.DEVNULL, check=True)
        group = []
        for rep in range(args.runs):
            cmd = base
            if rep == args.runs - 1:
                cmd = base + ['--field', str(fields / f'volume_{scene}_{n}_{kappa}.csv')]
            row = next(csv.DictReader(io.StringIO(subprocess.check_output(cmd, text=True))))
            assert float(row['div_rms']) < 1e-7, row
            assert float(row['mom_rms']) < 1e-7, row
            if scene == 'disk-moving':
                assert float(row['reaction_x']) < 0, row
            samples.append(dict(rep=rep, **row))
            group.append(row)
        times = sorted(float(row['solve_ms']) for row in group)
        summary = dict(group[-1])
        del summary['solve_ms']
        summary.update(runs=args.runs, solve_ms_mean=statistics.mean(times),
                       solve_ms_p95=times[math.ceil((len(times)-1)*.95)])
        summaries.append(summary)
        print(f'{scene} n={n} kappa={kappa}: mean={summary["solve_ms_mean"]:.3f} ms', flush=True)
for name, rows in [('volume-samples.csv', samples), ('volume-summary.csv', summaries)]:
    with (out / name).open('w') as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
print('Volume study complete.', flush=True)
