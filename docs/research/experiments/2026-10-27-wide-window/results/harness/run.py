#!/usr/bin/env python3
"""Compare existing two regions with recentered 48x48 / 96² regions in copies."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]
TARGET = Path('/tmp/sunyunbo/blood-stokes-rayon-target')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def replace(text, old, new):
    assert text.count(old) == 1, old
    return text.replace(old, new)


def prepare(source, folder, variant):
    shutil.copytree(source, folder)
    shutil.copy2(ROOT / 'probe.rs', folder / 'crates/simulation/examples/window_probe.rs')
    p = folder / 'crates/simulation/src/fluid/mod.rs'
    text = p.read_text()
    if variant == 'wide':
        text = replace(text, 'const N: usize = 64;', 'const N: usize = 96;')
        text = replace(text, 'const H: f64 = 0.25;', 'const H: f64 = 0.5;')
        text = replace(text, 'const ORIGIN: [f64; 2] = [-22.0, -8.0];',
                       'const ORIGIN: [f64; 2] = [-38.0, -24.0];')
        r = folder / 'crates/simulation/src/fluid/regions.rs'
        r.write_text(replace(r.read_text(), 'cells, [-18.0, -16.0])?',
                             'cells, [-34.0, -32.0])?'))
    text = replace(text, '        let mask = (0..N * N)',
                   '        let trial_init = Instant::now();\n        let mask = (0..N * N)')
    text = replace(text, '        let feedback = zero();',
                   '        eprintln!("WINDOW,{},{},{},{},{},{},{:.6}", origin[0], origin[1], N, H, '
                   'grid.velocity_count(), grid.cells.len(), trial_init.elapsed().as_secs_f64()*1000.0);\n'
                   '        let feedback = zero();')
    p.write_text(text)
    # Read-only diagnostic access, present only in both experimental copies.
    p = folder / 'crates/simulation/src/runtime.rs'
    p.write_text(p.read_text() + '''
impl Simulation {
    pub fn window_trial_details(&self) -> (usize, usize, f32, f32) {
        let local = self.local_fluid.as_ref().unwrap();
        let robot = self.physics.bodies[self.robot].translation();
        let half = Vector::new(1280.0 / (24.0 * 1.4) / 2.0, 800.0 / (24.0 * 1.4) / 2.0);
        let mut visible = 0;
        let mut full = 0;
        for &h in &self.cells {
            let c = fluid::center(&self.physics.soft_bodies[h]);
            if (c.x - robot.x).abs() <= half.x && (c.y - robot.y).abs() <= half.y {
                visible += 1;
                full += usize::from(local.weight(c, self.config.cell_radius) >= 0.99999);
            }
        }
        let [lo, hi] = local.stats().bounds;
        let margin = (robot.x - half.x - lo[0]).min(hi[0] - robot.x - half.x)
            .min(robot.y - half.y - lo[1]).min(hi[1] - robot.y - half.y);
        (visible, full, local.weight(robot, 0.85), margin)
    }

    pub fn window_trial_sample(&self, point: [f32; 2]) -> (f32, [f32; 2]) {
        let local = self.local_fluid.as_ref().unwrap();
        let p = Vector::from_array(point);
        let w = local.weight(p, self.config.cell_radius);
        (w, (self.environment.velocity_at(p) * (1.0-w) + local.sample(p) * w).to_array())
    }
}
''')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--seconds', type=float, default=20)
    parser.add_argument('--out', type=Path, default=ROOT / 'results')
    args = parser.parse_args()
    assert args.seconds >= 10 and {2, 3, 4, 5} <= os.sched_getaffinity(0)
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    paths = [REPO / 'Cargo.toml', REPO / 'Cargo.lock',
             *sorted((REPO / 'crates').rglob('*.rs')),
             *sorted((REPO / 'crates').rglob('Cargo.toml'))]
    hashes = {str(p.relative_to(REPO)): sha(p) for p in paths}
    for p in paths:
        dest = out / 'sources' / p.relative_to(REPO)
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p, dest)
    work = Path(tempfile.mkdtemp(prefix='blood-wide-window-', dir='/tmp/sunyunbo'))
    manifest = {'seconds': args.seconds, 'cpus': [2, 3, 4, 5], 'sources_sha256': hashes,
                'experiment_sha256': {n: sha(ROOT / n) for n in ['run.py', 'probe.rs', 'analyze.py']},
                'temporary_workspaces': str(work), 'audits': {}, 'commands': [], 'runs': [],
                'scene': '72 cells;2 substeps;6 analytic samples;4 fluid markers;two fixed centers [-14,0],[-10,-8];real waypoint input',
                'variants': {'base': {'side': 16, 'n': 64, 'h': .25},
                             'wide': {'side': 48, 'n': 96, 'h': .5}}}

    def save():
        (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')

    save()
    binaries = {}
    for variant in ['base', 'wide']:
        print('Preparing', variant, flush=True)
        folder = work / variant
        prepare(out / 'sources', folder, variant)
        changed = {str(p.relative_to(folder)): sha(p) for p in folder.rglob('*.rs')
                   if str(p.relative_to(folder)) not in hashes or sha(p) != hashes[str(p.relative_to(folder))]}
        for name in changed:
            dest = out / 'variants' / variant / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(folder / name, dest)
        manifest['variants'][variant]['modified_sha256'] = changed
        common = ['--release', '--offline', '--locked', '--manifest-path', str(folder / 'Cargo.toml'),
                  '-p', 'simulation', '--target-dir', str(TARGET), '-j', '4']
        command = ['cargo', 'test', *common]
        manifest['commands'].append(command)
        with (out / f'{variant}-audit.log').open('w') as log:
            result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
        manifest['audits'][variant] = result.returncode
        save()
        if result.returncode:
            raise SystemExit(f'{variant} existing checks failed; inspect audit log')
        command = ['cargo', 'build', *common, '--example', 'window_probe']
        manifest['commands'].append(command)
        save()
        with (out / f'{variant}-build.log').open('w') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
        binaries[variant] = work / f'{variant}-probe'
        shutil.copy2(TARGET / 'release/examples/window_probe', binaries[variant])
        manifest['variants'][variant]['binary_sha256'] = sha(binaries[variant])
        save()
    for name, variant in [('base-r1', 'base'), ('wide-r1', 'wide'),
                          ('wide-r2', 'wide'), ('base-r2', 'base')]:
        folder = out / name
        folder.mkdir()
        command = ['flock', '/tmp/blood-stokes-feedback-bench.lock', 'taskset', '-c', '2-5',
                   '/usr/bin/time', '-f', 'MAX_RSS_KIB,%M', str(binaries[variant]),
                   str(args.seconds), str(folder / 'drive')]
        print('Running', name, flush=True)
        with (folder / 'run.log').open('w') as log, (folder / 'stderr.log').open('w') as err:
            result = subprocess.run(command, stdout=log, stderr=err)
        manifest['runs'].append({'name': name, 'variant': variant, 'command': command,
                                 'exit_code': result.returncode})
        save()
        if result.returncode:
            raise SystemExit(f'{name} failed; raw data retained')
    environment = {'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
                   'cpu': subprocess.check_output(['lscpu'], text=True)}
    (out / 'environment.json').write_text(json.dumps(environment, indent=2) + '\n')
    assert all(sha(REPO / n) == h for n, h in hashes.items())
    print('Completed', out, flush=True)


if __name__ == '__main__':
    main()
