#!/usr/bin/env python3
"""Matched custom sparse / faer speed-inverse spike; production is untouched."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]
PREV = ROOT.parent / '2026-10-27-geometry-query'
SPEC = importlib.util.spec_from_file_location('geometry_trial', PREV / 'run.py')
GEOMETRY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GEOMETRY)
TRIAL = GEOMETRY.TRIAL


def prepare(folder, variant):
    GEOMETRY.prepare(folder, 'reuse')
    src = folder / 'crates/simulation/src/fluid'
    if variant == 'faer':
        shutil.copy2(ROOT / 'linear.rs', src / 'linear.rs')
    shutil.copy2(ROOT.parent / '2026-10-26-stokes-sparse/sparse.rs', src / 'custom_reference.rs')
    p = src / 'mod.rs'
    p.write_text(p.read_text() + '\n#[cfg(test)]\nmod custom_reference;\n')
    p = src / 'grid.rs'
    p.write_text(p.read_text() + '\n#[cfg(test)]\nmod faer_audit {\nuse super::*;\n'
                 + (ROOT / 'audit.rs').read_text() + '\n}\n')
    # Identical dependency/features in both builds; only adapter differs.
    p = folder / 'crates/simulation/Cargo.toml'
    p.write_text(p.read_text() + '\nfaer = { version = "=0.24.4", default-features = false, features = ["std", "sparse-linalg"] }\n')
    p = folder / 'crates/simulation/examples/runtime_probe.rs'
    old = '    let mut sim = Simulation::new_local_world(config);'
    text = p.read_text()
    assert text.count(old) == 1
    p.write_text(text.replace(old, '    let init_start = std::time::Instant::now();\n' + old
        + '\n    eprintln!("INIT_NS,{}",init_start.elapsed().as_nanos());'))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--seconds', type=float, default=30)
    parser.add_argument('--out', type=Path, default=ROOT / 'results')
    args = parser.parse_args()
    assert args.seconds > 0 and {2, 3, 4, 5} <= os.sched_getaffinity(0)
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    if (args.out / 'manifest.json').exists():
        raise SystemExit('Use a new --out; measurements are archived.')
    sources = dict(json.loads((PREV / 'results/manifest.json').read_text())['sources_sha256'])
    for name in ['run.py', 'linear.rs', 'audit.rs']:
        p = ROOT / name
        sources[str(p.relative_to(REPO))] = TRIAL.sha(p)
    assert all(TRIAL.sha(REPO / p) == h for p, h in sources.items())
    production = {str(p.relative_to(REPO)): TRIAL.sha(p) for p in
                  [REPO / 'Cargo.toml', REPO / 'Cargo.lock', *sorted((REPO / 'crates').rglob('*.rs')),
                   *sorted((REPO / 'crates').rglob('Cargo.toml'))]}
    work = Path(tempfile.mkdtemp(prefix='blood-faer-speed-', dir='/tmp/sunyunbo'))
    manifest = {'sources_sha256': sources, 'production_sha256': production,
                'temporary_workspaces': str(work), 'seconds': args.seconds,
                'cpus': [2, 3, 4, 5], 'commands': [], 'runs': [],
                'scene': '6 analytic samples;12 physical nodes;1e-4 Schur CG;geometry reuse;72 cells;2 substeps;fixed fork',
                'faer': '0.24.4;std+sparse-linalg;AMD+AUTO;Par::Seq within bounded outer Rayon join'}
    (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    binaries = {}
    for variant in ['custom', 'faer']:
        folder = work / variant
        prepare(folder, variant)
        old = tomllib.loads((folder / 'Cargo.lock').read_text())
        # First resolution may download new faer dependencies; subsequent steps locked/offline.
        command = ['cargo', 'fetch', '--manifest-path', str(folder / 'Cargo.toml')]
        manifest['commands'].append(command)
        print(variant, 'fetch', flush=True)
        with (args.out / f'{variant}-fetch.log').open('w') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
        new = tomllib.loads((folder / 'Cargo.lock').read_text())
        assert {(p['name'], p['version']) for p in old['package']} <= {(p['name'], p['version']) for p in new['package']}
        shutil.copy2(folder / 'Cargo.lock', args.out / f'{variant}-Cargo.lock')
        common = ['--release', '--offline', '--locked', '--manifest-path', str(folder / 'Cargo.toml'),
                  '-p', 'simulation', '--target-dir', str(TRIAL.TARGET), '-j', '4']
        # Restore strict pressure tolerance ONLY for existing strict fluid checks.
        grid = folder / 'crates/simulation/src/fluid/grid.rs'
        text = grid.read_text()
        target = 'let target = 0.0001 *'
        assert text.count(target) == 1
        grid.write_text(text.replace(target, 'let target = 2e-9 *'))
        command = ['cargo', 'test', *common, '--lib', 'fluid::', '--', '--nocapture']
        manifest['commands'].append(command)
        print(variant, 'strict fluid audits', flush=True)
        with (args.out / f'{variant}-audit.log').open('w') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
        grid.write_text(text)
        command = ['cargo', 'build', *common, '--example', 'runtime_probe']
        manifest['commands'].append(command)
        print(variant, 'build 1e-4 probe', flush=True)
        with (args.out / f'{variant}-build.log').open('w') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
        binaries[variant] = work / f'{variant}-probe'
        shutil.copy2(TRIAL.TARGET / 'release/examples/runtime_probe', binaries[variant])
    manifest['binary_sha256'] = {k: TRIAL.sha(v) for k, v in binaries.items()}
    for name, variant, mode in [('custom-r1', 'custom', 'none'), ('faer-r1', 'faer', 'none'),
                                ('faer-r2', 'faer', 'none'), ('custom-r2', 'custom', 'none'),
                                ('fluid-custom', 'custom', 'fluid'), ('fluid-faer', 'faer', 'fluid')]:
        out = args.out / name
        out.mkdir()
        env = dict(os.environ)
        env.pop('SPARSE_PROFILE', None)
        env.update(RUNTIME_PROFILE='1', DEEP_MODE=mode)
        command = ['flock', '/tmp/blood-stokes-feedback-bench.lock', 'taskset', '-c', '2-5',
                   '/usr/bin/time', '-f', 'MAX_RSS_KIB,%M', str(binaries[variant]), str(args.seconds), str(out)]
        print('Running', name, flush=True)
        with (out / 'run.log').open('w') as stdout, (out / 'stderr.log').open('w') as stderr:
            result = subprocess.run(command, stdout=stdout, stderr=stderr, env=env)
        manifest['runs'].append({'name': name, 'variant': variant, 'mode': mode,
                                 'command': command, 'exit_code': result.returncode})
        (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        if result.returncode:
            raise SystemExit(result.returncode)
    assert all(TRIAL.sha(REPO / p) == h for p, h in sources.items())
    assert all(TRIAL.sha(REPO / p) == h for p, h in production.items())
    print('Completed', args.out, flush=True)


if __name__ == '__main__':
    main()
