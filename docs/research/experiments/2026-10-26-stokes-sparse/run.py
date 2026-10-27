#!/usr/bin/env python3
"""Frozen matched simulation crates; sparse speed inverse is the only variant."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
TARGET = Path('/tmp/sunyunbo/blood-stokes-rayon-target')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare(folder, variant):
    shutil.copytree(ROOT / 'baseline', folder)
    source = folder / 'crates/simulation/src/fluid'
    if variant == 'sparse':
        shutil.copy2(ROOT / 'sparse.rs', source / 'linear.rs')
    else:
        path = source / 'linear.rs'
        text = path.read_text()
        pos = text.rfind('\n}')
        text = text[:pos] + '''
    pub fn factor_stats(&self) -> (usize, usize) {
        (self.lower.len(), self.lower.len() * 8)
    }
''' + text[pos:]
        path.write_text(text)
    # Common optional instrumentation; disabled for main timing trajectories.
    path = source / 'grid.rs'
    text = path.read_text().replace('const NONE: usize = usize::MAX;', '''
const NONE: usize = usize::MAX;
thread_local! {
    static PROFILE: std::cell::Cell<(usize, u128)> = const { std::cell::Cell::new((0,0)) };
}
''')
    text = text.replace('''            let factor = BandCholesky::new(&diagonal, &neighbors)?;''', '''
            let factor_start = std::time::Instant::now();
            let factor = BandCholesky::new(&diagonal, &neighbors)?;
            let factor_ns = factor_start.elapsed().as_nanos();
            let (entries, payload_bytes) = factor.factor_stats();
            eprintln!("FACTOR,{axis},{},{entries},{payload_bytes},{factor_ns}", faces.len());''')
    text = text.replace('''        let split = self.components[0].faces.len();''', '''
        let profiling = std::env::var_os("SPARSE_PROFILE").is_some();
        let start = profiling.then(std::time::Instant::now);
        let split = self.components[0].faces.len();''')
    text = text.replace('''        result.extend(other);
        result''', '''        result.extend(other);
        if let Some(start) = start {
            PROFILE.with(|p| { let (calls, ns) = p.get(); p.set((calls+1, ns+start.elapsed().as_nanos())); });
        }
        result''')
    text = text.replace('''        assert_eq!(force.len(), self.velocity_count());''', '''
        PROFILE.with(|p| p.set((0,0)));
        let start = std::time::Instant::now();
        assert_eq!(force.len(), self.velocity_count());''')
    text = text.replace('''        Ok(Field {
            divergence_rms,''', '''
        if std::env::var_os("SPARSE_PROFILE").is_some() {
            PROFILE.with(|p| { let (calls, ns)=p.get(); eprintln!("PROFILE,{iterations},{calls},{},{ns}",start.elapsed().as_nanos()); });
        }
        Ok(Field {
            divergence_rms,''')
    path.write_text(text)
    # Existing independent fluid checks plus one same-matrix comparison probe.
    shutil.copy2(ROOT / 'baseline/crates/simulation/src/fluid/linear.rs', source / 'reference.rs')
    with (source / 'mod.rs').open('a') as stream:
        stream.write('\n#[cfg(test)]\nmod reference;\n')
    with (source / 'grid.rs').open('a') as stream:
        stream.write('\n#[cfg(test)]\nmod inverse_comparison {\nuse super::*;\n')
        stream.write((ROOT / 'compare.rs').read_text())
        stream.write('\n}\n')
    # Leave the archived example unchanged; append trace capture in temp copy.
    path = folder / 'crates/simulation/examples/circulation_probe.rs'
    text = path.read_text().replace('''    let started = Instant::now();
    for _ in 0..steps {''', '''
    use std::fmt::Write as _;
    let mut frames = String::from("frame,whole_ms,fluid_ms,iterations,active,divergence,momentum,exchange,net_flux\\n");
    let mut cells = String::from("frame,cell,x,y,area\\n");
    let mut arrows = String::from("frame,arrow,x,y,vx,vy\\n");
    let started = Instant::now();
    for frame in 0..steps {''')
    text = text.replace('''        solve_times.push(state.solve_ms);''', '''
        if let Some(f) = &state.fluid {
            writeln!(frames, "{frame},{:.12},{:.12},{},{},{:.17e},{:.17e},{:.17e},{:.17e}",state.solve_ms,f.solve_ms,f.pressure_iterations,f.active_cells,f.divergence_rms,f.momentum_rms,f.exchange_error,f.net_flux).unwrap();
        }
        for (i,c) in state.cells.iter().enumerate() {
            let p = center(c);
            writeln!(cells,"{frame},{i},{:.9e},{:.9e},{:.9e}",p.x,p.y,area(c)).unwrap();
        }
        for (i,a) in state.arrows.iter().enumerate() {
            writeln!(arrows,"{frame},{i},{:.9e},{:.9e},{:.9e},{:.9e}",a.position[0],a.position[1],a.velocity[0],a.velocity[1]).unwrap();
        }
        solve_times.push(state.solve_ms);''')
    pos = text.rfind('\n}')
    text = text[:pos] + '''
    if let Some(out) = args.get(4) {
        let out = std::path::Path::new(out);
        std::fs::write(out.join("frames.csv"),frames).unwrap();
        std::fs::write(out.join("cells.csv"),cells).unwrap();
        std::fs::write(out.join("arrows.csv"),arrows).unwrap();
    }
''' + text[pos:]
    path.write_text(text)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', type=Path, default=ROOT / 'results')
    args = parser.parse_args()
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    if (args.out / 'manifest.json').exists():
        raise SystemExit('Use --out for a new measurement; archived results already exist.')
    before = {str(p.relative_to(ROOT)): sha(p) for p in sorted(ROOT.rglob('*'))
              if p.is_file() and not p.is_relative_to(args.out) and '__pycache__' not in p.parts}
    work = Path(tempfile.mkdtemp(prefix='blood-stokes-sparse-', dir='/tmp/sunyunbo'))
    manifest = {'sources_sha256': before, 'temporary_workspaces': str(work),
                'platform': platform.platform(), 'rustc': subprocess.check_output(['rustc','--version'],text=True).strip(),
                'cpu_model': next(s.split(':',1)[1].strip() for s in Path('/proc/cpuinfo').read_text().splitlines() if s.startswith('model name')),
                'scene': '72 cells; real 64^2 fixed window; 60 frames; 2 substeps; no warmup removal',
                'cpus': [2,3,4,5], 'commands': [], 'runs': []}
    assert {2,3,4,5} <= os.sched_getaffinity(0)
    for variant in ['band', 'sparse']:
        folder = work / variant
        prepare(folder, variant)
        common = ['--release','--offline','--locked','--manifest-path',str(folder/'Cargo.toml'),
                  '-p','simulation','--target-dir',str(TARGET)]
        for label, command in [('tests',['cargo','test',*common,'--lib','fluid::','--','--nocapture']),
                               ('build',['cargo','build',*common,'--example','circulation_probe'])]:
            print(variant, label, flush=True)
            manifest['commands'].append(command)
            with (args.out / f'{variant}-{label}.log').open('w') as stream:
                subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT, check=True)
        shutil.copy2(TARGET/'release/examples/circulation_probe', work/f'{variant}-probe')
    cases = [('band-r1','band',False),('sparse-r1','sparse',False),
             ('sparse-r2','sparse',False),('band-r2','band',False),
             ('profile-band','band',True),('profile-sparse','sparse',True)]
    manifest['binary_sha256'] = {v: sha(work/f'{v}-probe') for v in ['band','sparse']}
    for name, variant, profiling in cases:
        out = args.out / name
        out.mkdir()
        env = dict(os.environ)
        env.pop('SPARSE_PROFILE', None)
        if profiling:
            env['SPARSE_PROFILE'] = '1'
        command = ['flock','/tmp/blood-stokes-feedback-bench.lock','taskset','-c','2-5',
                   str(work/f'{variant}-probe'),'72','1','local',str(out)]
        print('Running',name,flush=True)
        with (out/'run.log').open('w') as stdout, (out/'profile.log').open('w') as stderr:
            proc = subprocess.run(command, stdout=stdout, stderr=stderr, env=env)
        manifest['runs'].append({'name':name,'variant':variant,'profiling':profiling,
                                 'command':command,'exit_code':proc.returncode})
        (args.out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
        if proc.returncode:
            raise SystemExit(proc.returncode)
    assert all(sha(ROOT / p) == h for p,h in before.items())
    print('Completed', args.out, flush=True)


if __name__ == '__main__':
    main()
