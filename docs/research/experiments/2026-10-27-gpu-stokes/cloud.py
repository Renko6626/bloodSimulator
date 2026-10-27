#!/usr/bin/env python3
"""Cloud handoff: provenance checks, CPU reference, hardware probe and GPU runs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--data', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--target-dir', type=Path, default=Path(tempfile.gettempdir()) / 'blood-gpu-lab-target')
    parser.add_argument('--cpu-only', action='store_true')
    parser.add_argument('--offline', action='store_true')
    parser.add_argument('--limit', type=int, default=4)
    parser.add_argument('--rounds', type=int, default=2)
    parser.add_argument('--restart', type=int, default=32)
    parser.add_argument('--max-iter', type=int, default=400)
    parser.add_argument('--rtol', type=float, default=1e-4)
    args = parser.parse_args()
    assert args.limit > 0 and args.rounds > 0 and 0 < args.restart <= 64 and args.max_iter > 0
    data = args.data.resolve()
    manifest = json.loads((data / 'manifest.json').read_text())
    assert manifest['format_version'] == 1
    for name, expected in manifest['sources_sha256'].items():
        assert sha(data / 'sources' / name) == expected, name
        assert sha(REPO / name) == expected, f'Current source differs from capture: {name}; recapture before comparison.'
    for name, expected in manifest['harness_sha256'].items():
        assert sha(data / 'harness' / name) == expected, name
    for name, info in manifest['cases'].items():
        assert info['exit_code'] == 0, name
        for file, expected in info['data_sha256'].items():
            assert sha(data / name / file) == expected, (name, file)
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    build = ['cargo', 'build', '--release', '--locked', '--manifest-path', str(ROOT / 'lab/Cargo.toml'),
             '--target-dir', str(args.target_dir), '-j', '4']
    if args.offline:
        build.append('--offline')
    print('Building independent GPU lab', flush=True)
    with (out / 'build.log').open('w') as log:
        subprocess.run(build, stdout=log, stderr=subprocess.STDOUT, check=True)
    binary = args.target_dir / ('release/blood-gpu-stokes-lab.exe' if os.name == 'nt' else 'release/blood-gpu-stokes-lab')
    sources = [ROOT / 'cloud.py', ROOT / 'prepare.py', ROOT / 'capture.rs', ROOT / 'driver.rs',
               ROOT / 'lab/Cargo.toml', ROOT / 'lab/Cargo.lock',
               *sorted((ROOT / 'lab/src').glob('*')),
               REPO / 'crates/simulation/src/fluid/linear.rs']
    records = {'format_version': 1, 'build_command': build, 'binary_sha256': sha(binary),
               'lab_sources_sha256': {str(p.relative_to(REPO)): sha(p) for p in sources},
               'dataset_manifest_sha256': sha(data / 'manifest.json'), 'data': str(data),
               'platform': platform.platform(), 'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
               'cpu_only': args.cpu_only, 'runs': [], 'hardware_tests_run': False}

    def save():
        (out / 'manifest.json').write_text(json.dumps(records, indent=2) + '\n')

    def run(name, arguments):
        command = [str(binary), *arguments, '--out', str(out / f'{name}.json')]
        print('Running', name, flush=True)
        with (out / f'{name}.stdout.log').open('w') as log, (out / f'{name}.stderr.log').open('w') as err:
            result = subprocess.run(command, stdout=log, stderr=err)
        record = {'name': name, 'command': command, 'exit_code': result.returncode}
        if (out / f'{name}.json').exists():
            record['result'] = json.loads((out / f'{name}.json').read_text())
        records['runs'].append(record)
        save()
        return result.returncode == 0

    save()
    gpu_ready = False
    if not args.cpu_only:
        gpu_ready = run('probe', ['probe'])
        if gpu_ready:
            gpu_ready = run('smoke', ['smoke'])
        records['hardware_tests_run'] = gpu_ready
    domains = [(name, folder) for name in manifest['cases']
               for folder in sorted((data / name).glob('region-*')) if folder.is_dir()]
    assert domains
    for name, folder in domains:
        label = f'{name}-{folder.name}'
        run(label + '-cpu', ['cpu', '--case', str(folder), '--limit', str(args.limit),
                            '--rtol', str(args.rtol), '--repeats', '10'])
    if gpu_ready:
        for round_id in range(1, args.rounds + 1):
            order = domains if round_id % 2 else list(reversed(domains))
            for name, folder in order:
                run(f'{name}-{folder.name}-gpu-r{round_id}',
                    ['gpu', '--case', str(folder), '--limit', str(args.limit),
                     '--rtol', str(args.rtol), '--restart', str(args.restart), '--max-iter', str(args.max_iter)])
    success = all(r['exit_code'] == 0 for r in records['runs'])
    if not args.cpu_only and not gpu_ready:
        success = False
    records['completed_successfully'] = success
    save()
    summary = {'completed_successfully': success, 'cpu_only': args.cpu_only,
               'hardware_tests_run': records['hardware_tests_run'],
               'runs': [{'name': r['name'], 'exit_code': r['exit_code'],
                         'accepted': r.get('result', {}).get('accepted', False)} for r in records['runs']],
               'notes': 'Nonconvergence is a retained negative result, not a successful GPU speedup. CPU LU is f64; GPU GMRES is f32. Timings exclude Rapier/rendering.'}
    (out / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    assert all(sha(REPO / name) == value for name, value in records['lab_sources_sha256'].items())
    print(json.dumps(summary, indent=2))
    raise SystemExit(0 if success else 1)


if __name__ == '__main__':
    main()
