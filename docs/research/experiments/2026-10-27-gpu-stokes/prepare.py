#!/usr/bin/env python3
"""Export production matrices/RHSs from copies; never edit production source."""
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
CASES = {'small64': (64, .25, [-22., -8.], [-18., -16.]),
         'wide96': (96, .5, [-38., -24.], [-34., -32.]),
         'wide192': (192, .25, [-38., -24.], [-34., -32.])}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def replace(text, old, new):
    assert text.count(old) == 1, old
    return text.replace(old, new)


def prepare(source, folder, case):
    shutil.copytree(source, folder)
    src = folder / 'crates/simulation/src/fluid'
    shutil.copy2(ROOT / 'capture.rs', src / 'capture.rs')
    shutil.copy2(ROOT / 'driver.rs', folder / 'crates/simulation/examples/gpu_capture.rs')
    n, h, origin, second = CASES[case]
    p = src / 'mod.rs'
    text = replace(p.read_text(), 'const N: usize = 64;', f'const N: usize = {n};')
    text = replace(text, 'const H: f64 = 0.25;', f'const H: f64 = {h};')
    text = replace(text, 'const ORIGIN: [f64; 2] = [-22.0, -8.0];',
                   f'const ORIGIN: [f64; 2] = {origin};')
    p.write_text(text)
    p = src / 'regions.rs'
    p.write_text(replace(p.read_text(), 'cells, [-18.0, -16.0])?', f'cells, {second})?'))
    p = src / 'linear.rs'
    text = p.read_text() + '\n#[path = "capture.rs"]\nmod capture;\n'
    text = replace(text, '    velocity_count: usize,\n}',
                   '    velocity_count: usize,\n    trial_capture: Option<capture::Capture>,\n}')
    text = replace(text, '        Ok(Self {',
                   '        let trial_capture = capture::Capture::new(velocity_count, pressure_count, entries)?;\n        Ok(Self {')
    text = replace(text, '            velocity_count,\n', '            velocity_count,\n            trial_capture,\n')
    text = replace(text, '        let mut result = rhs.to_vec();',
                   '        let trial_start = std::time::Instant::now();\n        let mut result = rhs.to_vec();')
    text = replace(text, '        result\n',
                   '        if let Some(c) = &self.trial_capture { c.record(rhs, &result, trial_start.elapsed().as_secs_f64()*1000.0); }\n        result\n')
    text += '''
impl StokesFactor {
    pub(super) fn trial_domain(&self, n: usize, h: f64, origin: [f64;2], cells: &[usize], u: &[[f64;2]], v: &[[f64;2]]) {
        if let Some(c) = &self.trial_capture { c.domain(n,h,origin,cells,u,v); }
    }
}
'''
    p.write_text(text)
    p = src / 'grid.rs'
    p.write_text(replace(p.read_text(), '        Ok(Self {\n            n,',
                         '        factor.trial_domain(n,h,origin,&cells,\n'
                         '            &components[0].faces.iter().map(|f| f.position).collect::<Vec<_>>(),\n'
                         '            &components[1].faces.iter().map(|f| f.position).collect::<Vec<_>>());\n'
                         '        Ok(Self {\n            n,'))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--frames', type=int, default=240)
    parser.add_argument('--cases', nargs='+', choices=CASES, default=list(CASES))
    parser.add_argument('--offline', action='store_true')
    parser.add_argument('--target-dir', type=Path, default=Path(tempfile.gettempdir()) / 'blood-gpu-capture-target')
    args = parser.parse_args()
    assert args.frames >= 1
    out = args.out.resolve()
    out.mkdir(parents=True, exist_ok=False)
    paths = [REPO / 'Cargo.toml', REPO / 'Cargo.lock',
             *sorted((REPO / 'crates').rglob('*.rs')),
             *sorted((REPO / 'crates').rglob('Cargo.toml'))]
    sources = {str(p.relative_to(REPO)): sha(p) for p in paths}
    for p in paths:
        dest = out / 'sources' / p.relative_to(REPO)
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p, dest)
    for name in ['prepare.py', 'capture.rs', 'driver.rs']:
        dest = out / 'harness' / name
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / name, dest)
    manifest = {'format_version': 1, 'frames': args.frames, 'sources_sha256': sources,
                'harness_sha256': {n: sha(ROOT/n) for n in ['prepare.py', 'capture.rs', 'driver.rs']},
                'cases': {}, 'known_issue': 'wide domains omit internal pump driving; linear algebra dataset only'}

    def save():
        (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')

    save()
    for case in args.cases:
        print('Preparing', case, flush=True)
        folder = out / 'workspaces' / case
        folder.parent.mkdir(exist_ok=True)
        prepare(out / 'sources', folder, case)
        command = ['cargo', 'build', '--release', '--locked', '--manifest-path', str(folder / 'Cargo.toml'),
                   '-p', 'simulation', '--example', 'gpu_capture', '--target-dir', str(args.target_dir), '-j', '4']
        if args.offline:
            command.append('--offline')
        data = out / case
        data.mkdir()
        info = manifest['cases'][case] = {'n': CASES[case][0], 'h': CASES[case][1],
                                       'commands': [command], 'exit_code': None}
        save()
        with (data / 'build.log').open('w') as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True)
        binary = args.target_dir / 'release/examples/gpu_capture'
        info['binary_sha256'] = sha(binary)
        command = [str(binary), str(args.frames)]
        info['commands'].append(command)
        env = dict(os.environ, BLOOD_GPU_CAPTURE=str(data))
        with (data / 'capture.log').open('w') as log:
            run = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, env=env)
        info['exit_code'] = run.returncode
        info['data_sha256'] = {str(p.relative_to(data)): sha(p) for p in sorted(data.rglob('*'))
                               if p.is_file() and p.suffix in ['.json', '.mtx', '.u32', '.f64', '.csv']}
        save()
        if run.returncode:
            raise SystemExit(f'{case} capture failed; inspect {data}/capture.log')
    assert all(sha(REPO / name) == value for name, value in sources.items())
    print('Captured', out, flush=True)


if __name__ == '__main__':
    main()
