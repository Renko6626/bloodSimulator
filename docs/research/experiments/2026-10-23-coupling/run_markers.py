"""Compare finite-resistance blobs and the membrane reference, serially."""
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
cases = [('blob', 32, m, drag) for m in (1, 4) for drag in (.25, 1, 4, 16)]
cases += [('blob', 64, 4, 4)]
cases += [('circle', 32, m, 32) for m in (12, 24, 48)]
metadata = dict(utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                rustc=subprocess.check_output(['rustc', '--version'], text=True).strip(),
                cpu=args.cpu, runs=args.runs, warmup_runs=1, exclusive=False,
                lscpu=subprocess.check_output(['lscpu'], text=True), compile_flags=['-O'],
                sha256={name: hashlib.sha256((ROOT / 'markers' / name).read_bytes()).hexdigest()
                        for name in ('main.rs', 'separable.rs')})
(out / 'markers-environment.json').write_text(json.dumps(metadata, indent=2))
samples, summaries = [], []
with tempfile.TemporaryDirectory(prefix='mac-markers-') as tmp:
    binary = str(Path(tmp) / 'probe')
    subprocess.run(['rustc', '-O', str(ROOT / 'markers/main.rs'), '-o', binary], check=True)
    for shape, n, m, parameter in cases:
        base = ['taskset', '-c', str(args.cpu), binary, str(n), str(m), str(fields),
                '--shape', shape, '--drag' if shape == 'blob' else '--kappa', str(parameter)]
        subprocess.run(base, stdout=subprocess.DEVNULL, check=True)
        groups = {'self': [], 'lagged': []}
        for rep in range(args.runs):
            rows = list(csv.DictReader(io.StringIO(subprocess.check_output(base, text=True))))
            assert len(rows) == 2
            for row in rows:
                assert float(row['div_rms']) < 1e-7, row
                assert float(row['momentum_rms']) < 1e-7, row
                assert float(row['force_balance_abs']) < 1e-9, row
                if row['kind'] == 'self':
                    assert float(row['force_law_rms']) < 1e-8, row
                samples.append(dict(rep=rep, **row))
                groups[row['kind']].append(row)
        for kind, group in groups.items():
            summary = dict(group[-1])
            summary['runs'] = args.runs
            for key in ('assemble_ms', 'factor_ms', 'final_fluid_ms', 'total_ms'):
                values = sorted(float(row[key]) for row in group)
                del summary[key]
                summary[key+'_mean'] = statistics.mean(values)
                summary[key+'_p95'] = values[math.ceil((len(values)-1)*.95)]
            summaries.append(summary)
        print(f'{shape} n={n} m={m} parameter={parameter}: '
              f'self={summaries[-2]["total_ms_mean"]:.3f} ms, '
              f'lag update={summaries[-1]["total_ms_mean"]:.3f} ms', flush=True)
for name, rows in [('markers-samples.csv', samples), ('markers-summary.csv', summaries)]:
    with (out / name).open('w') as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
print('Marker study complete.', flush=True)
