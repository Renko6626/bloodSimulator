#!/usr/bin/env python3
"""Replay unchanged portable cases with a prebuilt, hash-checked Linux binary."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parent
OUT = ROOT / 'results'
OUT.mkdir(exist_ok=False)
BINARY = ROOT / 'bin/fluid-bench'
ENV = dict(os.environ, XDG_RUNTIME_DIR=str(ROOT / 'runtime'), WGPU_BACKEND='vulkan')
Path(ENV['XDG_RUNTIME_DIR']).mkdir(mode=0o700, exist_ok=True)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    try:
        return Path(path).read_text()
    except OSError as error:
        return str(error)


def command(args):
    result = subprocess.run(args, capture_output=True, text=True, env=ENV)
    return {'command': args, 'exit_code': result.returncode,
            'stdout': result.stdout, 'stderr': result.stderr}


def hardware():
    return {'time_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
            'platform': platform.platform(),
            'cpu': command(['lscpu']),
            'gpu': command(['nvidia-smi', '--query-gpu=name,driver_version,pstate,temperature.gpu,power.draw,clocks.current.graphics,clocks.current.memory,utilization.gpu,memory.used', '--format=csv']),
            'gpu_processes': command(['nvidia-smi']),
            'vulkan': command(['vulkaninfo', '--summary']),
            'cpu_quota': read('/sys/fs/cgroup/cpu.max'),
            'cpu_set': read('/sys/fs/cgroup/cpuset.cpus.effective'),
            'memory_limit': read('/sys/fs/cgroup/memory.max'),
            'cpu_stat': read('/sys/fs/cgroup/cpu.stat'),
            'loadavg': read('/proc/loadavg')}


build = json.loads((ROOT / 'build.json').read_text())
assert sha(BINARY) == build['binary_sha256']
assert sha(ROOT / 'source.tar') == build['source_archive_sha256']
for path, digest in build['dataset_sha256'].items():
    assert sha(ROOT / 'data' / path) == digest, path
manifest = {'schema_version': 2, 'build': build,
            'harness_sha256': sha(Path(__file__)),
            'source_sha256': {str(p.relative_to(ROOT / 'source')): sha(p)
                              for p in sorted((ROOT / 'source').rglob('*')) if p.is_file()},
            'hardware_measurement': False,
            'options': {'restart': 32, 'max_iterations': 200, 'tolerance': 1e-4,
                        'limit': 4, 'rounds': 2, 'cpu_lu_repeats': 10,
                        'v_cycles': [1, 2], 'input': 'portable Linux capture, byte-identical to prior Windows data'},
            'hardware_before': hardware(), 'runs': [],
            'notes': 'Prebuilt Linux executable; no cloud compilation. Interleaved c1/c2; reversed domain/backend order on round 2. No explicit CPU affinity or GPU clock locking. Replay only, excluding Rapier/rendering.'}


def save():
    (OUT / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')


def run(name, args):
    full = [str(BINARY), *args, '--out', str(OUT / f'{name}.json')]
    before = read('/sys/fs/cgroup/cpu.stat')
    print('Running', name, flush=True)
    with (OUT / f'{name}.stdout.log').open('w') as log, (OUT / f'{name}.stderr.log').open('w') as error:
        result = subprocess.run(full, stdout=log, stderr=error, env=ENV)
    record = {'name': name, 'command': full, 'exit_code': result.returncode,
              'cpu_stat_before': before, 'cpu_stat_after': read('/sys/fs/cgroup/cpu.stat')}
    path = OUT / f'{name}.json'
    if path.exists():
        record['result'] = json.loads(path.read_text())
    manifest['runs'].append(record)
    save()
    return result.returncode == 0


save()
if not run('probe', ['probe']) or not run('smoke', ['smoke']):
    raise SystemExit('Hardware probe or known-solution check failed; inspect results')
assert manifest['runs'][0]['result']['adapter']['software'] is False
manifest['hardware_measurement'] = True
domains = [ROOT / 'data' / case / f'region-{region}'
           for case in ['small64', 'wide96', 'wide192'] for region in [0, 1]]
configs = [('cpu-lu', 2), ('cpu-fgmres-mg', 1), ('gpu-fgmres-mg', 1),
           ('cpu-fgmres-mg', 2), ('gpu-fgmres-mg', 2)]
for round_id in [1, 2]:
    for index, folder in enumerate(domains if round_id == 1 else list(reversed(domains))):
        order = configs if (index + round_id) % 2 else list(reversed(configs))
        for backend, cycles in order:
            name = f'{folder.parent.name}-{folder.name}-{backend}-c{cycles}-r{round_id}'
            run(name, ['run', '--backend', backend, '--case', str(folder), '--limit', '4',
                       '--restart', '32', '--max-iter', '200', '--rtol', '1e-4',
                       '--mg-cycles', str(cycles), '--repeats', '10' if backend == 'cpu-lu' else '1'])
manifest['hardware_after'] = hardware()
manifest['completed_successfully'] = all(r['exit_code'] == 0 for r in manifest['runs'])
assert sha(BINARY) == build['binary_sha256']
save()
summary = {'completed_successfully': manifest['completed_successfully'],
           'hardware_measurement': True,
           'runs': [{'name': r['name'], 'exit_code': r['exit_code'],
                     'accepted': r.get('result', {}).get('accepted')} for r in manifest['runs']]}
(OUT / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
print(json.dumps(summary, indent=2))
raise SystemExit(0 if manifest['completed_successfully'] else 1)
