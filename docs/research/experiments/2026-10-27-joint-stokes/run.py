#!/usr/bin/env python3
"""Matched Schur-CG / monolithic Stokes LU, frozen temporary workspaces."""
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
LOCK = ROOT.parent / '2026-10-27-faer-speed/results/custom-Cargo.lock'


def replace(text, old, new):
    assert text.count(old) == 1, old
    return text.replace(old, new)


def prepare(folder, variant):
    GEOMETRY.prepare(folder, 'reuse')
    old_lock = tomllib.loads((folder / 'Cargo.lock').read_text())
    new_lock = tomllib.loads(LOCK.read_text())
    assert {(p['name'], p['version']) for p in old_lock['package']} <= {(p['name'], p['version']) for p in new_lock['package']}
    shutil.copy2(LOCK, folder / 'Cargo.lock')
    p = folder / 'crates/simulation/Cargo.toml'
    p.write_text(p.read_text() + '\nfaer = { version = "=0.24.4", default-features = false, features = ["std", "sparse-linalg"] }\n')
    src = folder / 'crates/simulation/src'
    p = src / 'profile.rs'
    text = replace(p.read_text(), 'pub const NAMES: [&str; 43]', 'pub const NAMES: [&str; 44]')
    text = replace(text, '    "rapier_writeback",', '    "rapier_writeback",\n    "joint_solve",')
    text = replace(text, '} else if id < 30 {', '} else if id < 30 || id >= 43 {')
    p.write_text(text)
    p = folder / 'crates/simulation/examples/runtime_probe.rs'
    old = '    let mut sim = Simulation::new_local_world(config);'
    p.write_text(replace(p.read_text(), old,
                        '    let init_start=std::time::Instant::now();\n' + old
                        + '\n    eprintln!("INIT_NS,{}",init_start.elapsed().as_nanos());'))
    if variant == 'joint':
        shutil.copy2(ROOT / 'joint.rs', src / 'fluid/joint.rs')
        p = src / 'fluid/mod.rs'
        p.write_text(p.read_text() + '\nmod joint;\n')
        p = src / 'fluid/grid.rs'
        text = replace(p.read_text(), '    factor: BandCholesky,', '    #[cfg(test)]\n    factor: BandCholesky,')
        begin = text.index('            let factor_start =')
        end = text.index('            Ok(Component {', begin)
        setup = text[begin:end]
        text = text[:begin] + '            #[cfg(test)]\n            let factor = {\n' + setup + '\n                factor\n            };\n' + text[end:]
        text = replace(text, '                factor,', '                #[cfg(test)]\n                factor,')
        text = replace(text, '    components: [Component; 2],', '    components: [Component; 2],\n    joint: super::joint::JointLu,')
        text = replace(text, '        Ok(Self {\n            n,',
                       '        let components=[u?,v?];\n'
                       '        let joint=Self::build_joint(&components,cells.len(),h)?;\n'
                       '        Ok(Self {\n            n,')
        text = replace(text, '            components: [u?, v?],', '            components,\n            joint,')
        text = replace(text, '    fn inverse(&self, rhs: &[f64])', '    #[cfg(test)]\n    fn inverse(&self, rhs: &[f64])')
        text = replace(text, '    fn schur(&self, pressure: &[f64])', '    #[cfg(test)]\n    fn schur(&self, pressure: &[f64])')
        begin = text.index('    pub fn solve(\n')
        end = min(text.index('    pub fn long_diagnostics(', begin),
                  text.index('    pub fn boundary_rhs(', begin))
        old = text[begin:end]
        reference = replace(old, '    pub fn solve(', '    #[cfg(test)]\n    fn solve_cg_reference(')
        a = old.index('        let mut pressure =')
        b = old.index('        let (divergence_rms, momentum_rms, net_flux)', a)
        direct = old[:a] + '        let (velocity,pressure)=self.joint.solve(&rhs);\n        let iterations=0;\n' + old[b:]
        direct = replace(direct, '        prior: Option<&[f64]>,', '        _prior: Option<&[f64]>,')
        for line in ['        let mut mean = 0.0;\n', '        let mut count = 0;\n',
                     '                mean += p;\n', '                count += 1;\n']:
            direct = replace(direct, line, '')
        text = text[:begin] + direct + reference + text[end:]
        text += '\n' + (ROOT / 'assembly.rs').read_text()
        text += '\n#[cfg(test)]\nmod joint_audit {\nuse super::*;\n' + (ROOT / 'audit.rs').read_text() + '\n}\n'
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
    for p in [ROOT / n for n in ['run.py', 'joint.rs', 'assembly.rs', 'audit.rs']] + [LOCK]:
        sources[str(p.relative_to(REPO))] = TRIAL.sha(p)
    production = {str(p.relative_to(REPO)): TRIAL.sha(p) for p in
                  [REPO / 'Cargo.toml', REPO / 'Cargo.lock', *sorted((REPO / 'crates').rglob('*.rs')),
                   *sorted((REPO / 'crates').rglob('Cargo.toml'))]}
    assert all(TRIAL.sha(REPO / p) == h for p, h in sources.items())
    work = Path(tempfile.mkdtemp(prefix='blood-joint-stokes-', dir='/tmp/sunyunbo'))
    manifest = {'sources_sha256': sources, 'production_sha256': production,
                'temporary_workspaces': str(work), 'seconds': args.seconds, 'cpus': [2, 3, 4, 5],
                'scene': '6 analytic samples;12 physical nodes;geometry reuse;72 cells;2 substeps;CG 1e-4 vs direct f64 LU',
                'candidate': 'K=[A,-B^T;-B,0];faer 0.24.4 default COLAMD/AUTO pivoted sparse LU;Par::Seq;fixed factors reused',
                'commands': [], 'runs': [], 'audits': {}}

    def save():
        (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')

    save()
    binaries = {}
    for variant in ['cg', 'joint']:
        folder = work / variant
        prepare(folder, variant)
        common = ['--release', '--offline', '--locked', '--manifest-path', str(folder / 'Cargo.toml'),
                  '-p', 'simulation', '--target-dir', str(TRIAL.TARGET), '-j', '4']
        grid = folder / 'crates/simulation/src/fluid/grid.rs'
        dynamic = grid.read_text()
        # In the joint copy this target exists only in the cfg(test) CG reference.
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
    for name, variant, mode in [('cg-r1', 'cg', 'none'), ('joint-r1', 'joint', 'none'),
                                ('joint-r2', 'joint', 'none'), ('cg-r2', 'cg', 'none'),
                                ('fluid-cg', 'cg', 'fluid'), ('fluid-joint', 'joint', 'fluid')]:
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
        save()
        if result.returncode:
            raise SystemExit(result.returncode)
    assert all(TRIAL.sha(REPO / p) == h for p, h in sources.items())
    assert all(TRIAL.sha(REPO / p) == h for p, h in production.items())
    print('Completed', args.out, flush=True)


if __name__ == '__main__':
    main()
