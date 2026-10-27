#!/usr/bin/env python3
"""Small exact-S / SIMPLE-PCG spike; no new libraries or production edits."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]
PREV = ROOT.parent / '2026-10-27-geometry-query'
SPEC = importlib.util.spec_from_file_location('geometry_trial', PREV / 'run.py')
GEOMETRY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(GEOMETRY)
TRIAL = GEOMETRY.TRIAL


def replace(text, old, new):
    assert text.count(old) == 1, old
    return text.replace(old, new)


def prepare(folder, variant):
    GEOMETRY.prepare(folder, 'reuse')
    src = folder / 'crates/simulation/src'
    p = src / 'profile.rs'
    text = replace(p.read_text(), 'pub const NAMES: [&str; 43]', 'pub const NAMES: [&str; 44]')
    text = replace(text, '    "rapier_writeback",', '    "rapier_writeback",\n    "pressure_preconditioner",')
    text = replace(text, '} else if id < 30 {', '} else if id < 30 || id >= 43 {')
    p.write_text(text)
    p = folder / 'crates/simulation/examples/runtime_probe.rs'
    old = '    let mut sim = Simulation::new_local_world(config);'
    p.write_text(replace(p.read_text(), old,
                        '    let init_start=std::time::Instant::now();\n' + old
                        + '\n    eprintln!("INIT_NS,{}",init_start.elapsed().as_nanos());'))
    if variant == 'simple':
        # Reuse the same existing exact sparse factor for P; no extra inner CG,
        # no changing preconditioner, and no faer addition mixed into the trial.
        shutil.copy2(ROOT.parent / '2026-10-26-stokes-sparse/sparse.rs', src / 'fluid/pressure_linear.rs')
        p = src / 'fluid/mod.rs'
        p.write_text(p.read_text() + '\nmod pressure_linear;\n')
        p = src / 'fluid/grid.rs'
        text = replace(p.read_text(), '    components: [Component; 2],',
                       '    components: [Component; 2],\n    pressure_preconditioner: super::pressure_linear::BandCholesky,')
        text = replace(text, '        Ok(Self {\n            n,',
                       '        let components=[u?,v?];\n'
                       '        let pressure_preconditioner=Self::build_pressure_preconditioner(&components,cells.len(),h)?;\n'
                       '        Ok(Self {\n            n,')
        text = replace(text, '            components: [u?, v?],', '            components,\n            pressure_preconditioner,')
        text = replace(text, '        let mut direction = residual.clone();',
                       '        let mut direction = self.precondition(&residual);\n'
                       '        let mut rz=dot(&residual,&direction);')
        text = replace(text, '            let alpha = rr / denominator;', '            let alpha = rz / denominator;')
        text = replace(text, '''            for k in 0..direction.len() {
                direction[k] = residual[k] + next / rr * direction[k];
            }
            rr = next;''', '''            let z=self.precondition(&residual);
            let next_rz=dot(&residual,&z);
            for k in 0..direction.len() {
                direction[k] = z[k] + next_rz / rz * direction[k];
            }
            rz=next_rz;
            rr = next;''')
        text += '\n' + (ROOT / 'pressure.rs').read_text()
        text += '\n#[cfg(test)]\nmod simple_audit {\nuse super::*;\n' + (ROOT / 'audit.rs').read_text() + '\n}\n'
        p.write_text(text)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--seconds', type=float, default=6)
    parser.add_argument('--out', type=Path, default=ROOT / 'results')
    args = parser.parse_args()
    assert args.seconds > 0 and {2, 3, 4, 5} <= os.sched_getaffinity(0)
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    if (args.out / 'manifest.json').exists():
        raise SystemExit('Use a new --out; results are archived.')
    sources = dict(json.loads((PREV / 'results/manifest.json').read_text())['sources_sha256'])
    for name in ['run.py', 'pressure.rs', 'audit.rs']:
        p = ROOT / name
        sources[str(p.relative_to(REPO))] = TRIAL.sha(p)
    production = {str(p.relative_to(REPO)): TRIAL.sha(p) for p in
                  [REPO / 'Cargo.toml', REPO / 'Cargo.lock', *sorted((REPO / 'crates').rglob('*.rs')),
                   *sorted((REPO / 'crates').rglob('Cargo.toml'))]}
    assert all(TRIAL.sha(REPO / p) == h for p, h in sources.items())
    work = Path(tempfile.mkdtemp(prefix='blood-pressure-simple-', dir='/tmp/sunyunbo'))
    manifest = {'sources_sha256': sources, 'production_sha256': production,
                'temporary_workspaces': str(work), 'seconds': args.seconds, 'cpus': [2, 3, 4, 5],
                'scene': 'custom sparse velocity;6 analytic samples;12 physical nodes;1e-4;geometry reuse;72 cells;2 substeps',
                'candidate': 'exact S retained; fixed P=B diag(A)^-1 B^T; reuse custom sparse Cholesky to apply P^-1; PCG',
                'commands': [], 'runs': [], 'audits': {}}

    def save():
        (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')

    save()
    binaries = {}
    for variant in ['cg', 'simple']:
        folder = work / variant
        prepare(folder, variant)
        common = ['--release', '--offline', '--locked', '--manifest-path', str(folder / 'Cargo.toml'),
                  '-p', 'simulation', '--target-dir', str(TRIAL.TARGET), '-j', '4']
        grid = folder / 'crates/simulation/src/fluid/grid.rs'
        dynamic = grid.read_text()
        grid.write_text(replace(dynamic, 'let target = 0.0001 *', 'let target = 2e-9 *'))
        command = ['cargo', 'test', *common, '--lib', 'fluid::', '--', '--nocapture']
        manifest['commands'].append(command)
        print(variant, 'strict checks', flush=True)
        with (args.out / f'{variant}-audit.log').open('w') as log:
            result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
        manifest['audits'][variant] = result.returncode
        save()
        if result.returncode:
            raise SystemExit('Strict audit failed; no dynamic result claimed. Inspect archived log.')
        grid.write_text(dynamic)
        command = ['cargo', 'build', *common, '--example', 'runtime_probe']
        manifest['commands'].append(command)
        save()
        print(variant, 'build', flush=True)
        with (args.out / f'{variant}-build.log').open('w') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
        binaries[variant] = work / f'{variant}-probe'
        shutil.copy2(TRIAL.TARGET / 'release/examples/runtime_probe', binaries[variant])
    manifest['binary_sha256'] = {k: TRIAL.sha(v) for k, v in binaries.items()}
    for name, variant, mode in [('cg-r1', 'cg', 'none'), ('simple-r1', 'simple', 'none'),
                                ('simple-r2', 'simple', 'none'), ('cg-r2', 'cg', 'none'),
                                ('fluid-cg', 'cg', 'fluid'), ('fluid-simple', 'simple', 'fluid')]:
        out = args.out / name
        out.mkdir()
        env = dict(os.environ)
        env.pop('SPARSE_PROFILE', None)
        env.update(RUNTIME_PROFILE='1', DEEP_MODE=mode)
        command = ['flock', '/tmp/blood-stokes-feedback-bench.lock', 'taskset', '-c', '2-5',
                   str(binaries[variant]), str(args.seconds), str(out)]
        print('Running', name, flush=True)
        with (out / 'run.log').open('w') as stdout, (out / 'stderr.log').open('w') as stderr:
            result = subprocess.run(command, stdout=stdout, stderr=stderr, env=env)
        manifest['runs'].append({'name': name, 'variant': variant, 'mode': mode,
                                 'command': command, 'exit_code': result.returncode})
        save()
        if result.returncode:
            raise SystemExit(result.returncode)
    assert all(TRIAL.sha(REPO / p) == h for p, h in sources.items())
    assert all(TRIAL.sha(REPO / p) == h for p, h in production.items())
    print('Completed', args.out, flush=True)


if __name__ == '__main__':
    main()
