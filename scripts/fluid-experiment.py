#!/usr/bin/env python3
"""Run independently selectable fluid solver modules on portable real cases."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess

REPO = Path(__file__).resolve().parent.parent
BACKENDS = ['cpu-lu', 'cpu-fgmres-mg', 'gpu-gmres', 'gpu-fgmres-mg']


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def discover_domains(data, cases):
    domains = []
    for case in cases:
        for region in ['region-0', 'region-1']:
            folder = data / case / region
            if not folder.is_dir():
                raise ValueError(f'Missing requested domain: {folder}')
            domains.append((case, folder))
    return domains


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', type=Path)
    parser.add_argument('--data', type=Path, help='Reuse an existing directory of version-one cases')
    parser.add_argument('--backends', choices=BACKENDS, nargs='+', default=['cpu-lu', 'cpu-fgmres-mg', 'gpu-fgmres-mg'])
    parser.add_argument('--cases', choices=['small64', 'wide96', 'wide192'], nargs='+', default=['small64', 'wide96', 'wide192'])
    parser.add_argument('--cpu-only', action='store_true')
    parser.add_argument('--allow-software', action='store_true', help='Functional checks only; never hardware performance')
    parser.add_argument('--offline', action='store_true')
    parser.add_argument('--frames', type=int, default=240)
    parser.add_argument('--limit', type=int, default=4)
    parser.add_argument('--rounds', type=int, default=2)
    parser.add_argument('--restart', type=int, default=32)
    parser.add_argument('--max-iter', type=int, default=200)
    parser.add_argument('--mg-cycles', type=int, choices=[1, 2, 3, 4], default=2)
    parser.add_argument('--rtol', type=float, default=1e-4)
    args = parser.parse_args()
    assert args.limit > 0 and args.rounds > 0 and args.frames > 0
    if args.data:
        try:
            discover_domains(args.data, args.cases)
        except ValueError as error:
            parser.error(str(error))
    backends = [b for b in args.backends if not args.cpu_only or b.startswith('cpu-')]
    assert backends, 'No selected backends'
    gpu = any(b.startswith('gpu-') for b in backends)
    stamp = datetime.datetime.now().strftime('%Y%m%d-%H%M%S')
    out = (args.out or REPO / 'target/fluid-experiments' / stamp).resolve()
    out.mkdir(parents=True, exist_ok=False)
    target = Path(os.environ.get('CARGO_TARGET_DIR', REPO / 'target/fluid-bench')).resolve()
    command = ['cargo', 'build', '--release', '--locked', '-p', 'fluid-bench', '--target-dir', str(target), '-j', '4']
    if gpu:
        command += ['--features', 'gpu']
    if args.offline:
        command += ['--offline']
    print('Building fluid-bench', flush=True)
    with (out / 'build.log').open('w') as log:
        subprocess.run(command, cwd=REPO, stdout=log, stderr=subprocess.STDOUT, check=True)
    binary = target / ('release/fluid-bench.exe' if os.name == 'nt' else 'release/fluid-bench')
    files = [REPO / 'Cargo.toml', REPO / 'Cargo.lock', Path(__file__),
             *sorted((REPO / 'crates').rglob('*.rs')), *sorted((REPO / 'crates').rglob('*.wgsl')),
             *sorted((REPO / 'crates').rglob('Cargo.toml'))]
    manifest = {'schema_version': 2, 'platform': platform.platform(), 'build_command': command,
                'binary_sha256': sha(binary), 'source_sha256': {str(p.relative_to(REPO)): sha(p) for p in files},
                'options': vars(args).copy(), 'runs': [], 'hardware_measurement': False}
    manifest['options']['out'] = str(out)
    manifest['options']['data'] = str(args.data) if args.data else None

    def save():
        (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')

    def finish(success, error=None):
        manifest['completed_successfully'] = success
        if error:
            manifest['error'] = error
        save()
        summary = {'completed_successfully': success,
                   'hardware_measurement': manifest['hardware_measurement'],
                   'error': error,
                   'runs': [{'name': r['name'], 'exit_code': r['exit_code'],
                             'accepted': r.get('result', {}).get('accepted')} for r in manifest['runs']],
                   'notes': 'Solver replay only. Never infer game-frame cost or hardware speedup from failed/software runs.'}
        (out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
        print(json.dumps(summary, indent=2))
        raise SystemExit(0 if success else 1)

    def run(name, arguments, result=True):
        command = [str(binary), *arguments]
        if result:
            command += ['--out', str(out / f'{name}.json')]
        if args.allow_software and arguments[0] in ['probe', 'smoke', 'run']:
            command += ['--allow-software']
        print('Running', name, flush=True)
        with (out / f'{name}.stdout.log').open('w') as log, (out / f'{name}.stderr.log').open('w') as err:
            process = subprocess.run(command, cwd=REPO, stdout=log, stderr=err)
        record = {'name': name, 'command': command, 'exit_code': process.returncode}
        if result and (out / f'{name}.json').exists():
            record['result'] = json.loads((out / f'{name}.json').read_text())
        manifest['runs'].append(record)
        save()
        return process.returncode == 0

    save()
    if gpu:
        if not run('probe', ['probe']) or not run('smoke', ['smoke']):
            finish(False, 'GPU setup/known-solution check failed; inspect raw records')
        manifest['hardware_measurement'] = not manifest['runs'][0]['result']['adapter']['software']
    data = args.data.resolve() if args.data else out / 'data'
    if not args.data:
        for case in args.cases:
            if not run(f'capture-{case}', ['capture', '--case', case, '--frames', str(args.frames), '--out', str(data / case)], result=False):
                finish(False, 'Case capture failed; inspect raw records')
    try:
        domains = discover_domains(data, args.cases)
    except ValueError as error:
        finish(False, str(error))
    manifest['dataset_sha256'] = {str(p.relative_to(data)): sha(p) for _, folder in domains
                                 for p in sorted(folder.iterdir()) if p.is_file()}
    for round_id in range(1, args.rounds + 1):
        order = domains if round_id % 2 else list(reversed(domains))
        for case, folder in order:
            for backend in backends:
                run(f'{case}-{folder.name}-{backend}-r{round_id}',
                    ['run', '--backend', backend, '--case', str(folder), '--limit', str(args.limit),
                     '--restart', str(args.restart), '--max-iter', str(args.max_iter), '--rtol', str(args.rtol),
                     '--mg-cycles', str(args.mg_cycles),
                     '--repeats', '10' if backend == 'cpu-lu' else '1'])
    if not all(sha(REPO / name) == value for name, value in manifest['source_sha256'].items()):
        finish(False, 'Sources changed during the experiment')
    finish(all(r['exit_code'] == 0 for r in manifest['runs']))


if __name__ == '__main__':
    main()
