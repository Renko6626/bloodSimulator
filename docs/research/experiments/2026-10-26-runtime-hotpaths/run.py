#!/usr/bin/env python3
"""Profile the non-fluid remainder and test only zero-drag sampling removal."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT=Path(__file__).resolve().parent
SPARSE=ROOT.parent/'2026-10-26-stokes-sparse'
sys.path.insert(0,str(SPARSE))
from longrun import prepare_long
from run import TARGET, sha


STAGES='''
#[derive(Clone, Debug, Default)]
pub struct StageProfile {
    pub enabled: bool,
    pub network_ms: f64,
    pub prepare_ms: f64,
    pub robot_ms: f64,
    pub advection_ms: f64,
    pub fluid_ms: f64,
    pub rapier_ms: f64,
    pub analytic_samples: usize,
    pub zero_drag_nodes: usize,
    pub zero_drag_bodies: usize,
}
'''


def replace(text,old,new):
    assert text.count(old)==1,old
    return text.replace(old,new)


def start():
    return 'let phase_start=self.stages.enabled.then(Instant::now);\n'


def end(name):
    return f'if let Some(t)=phase_start {{ self.stages.{name}_ms+=t.elapsed().as_secs_f64()*1000.0; }}\n'


def prepare(folder,variant):
    prepare_long(folder,1e-4)
    source=folder/'crates/simulation/src'
    p=source/'runtime.rs'
    text=p.read_text()
    text=replace(text,'    local_fluid: Option<LocalFluid>,','    local_fluid: Option<LocalFluid>,\n    stages: StageProfile,')
    text=replace(text,'            local_fluid: None,','            local_fluid: None,\n            stages: StageProfile::default(),')
    text=replace(text,'        let started = Instant::now();','''
        self.stages=StageProfile { enabled: std::env::var_os("RUNTIME_PROFILE").is_some(), ..StageProfile::default() };
        let started = Instant::now();''')
    text=replace(text,'            solve_ms: self.solve_ms,','            solve_ms: self.solve_ms,\n            stages: self.stages.clone(),')
    a=text.index('    fn step_inner(')
    b=text.index('    pub fn snapshot(',a)
    step=text[a:b]
    step=replace(step,'        self.environment.update(pressure);',start()+'self.environment.update(pressure);\n'+end('network')+start())
    step=replace(step,'        let dt = self.config.dt / self.config.substeps as f32;',end('prepare')+'let dt = self.config.dt / self.config.substeps as f32;')
    step=replace(step,'            let mut flow_velocity = self',start()+'let mut flow_velocity = self')
    step=replace(step,'            for &handle in &self.cells {',end('robot')+start()+'for &handle in &self.cells {')
    step=replace(step,'                cell::advect(','''
                let samples=std::cell::Cell::new(0_usize);
                let profiling=self.stages.enabled;
                if profiling && self.config.cell_drag*(1.0-local_weight)==0.0 {
                    self.stages.zero_drag_nodes+=self.physics.soft_bodies[handle].num_particles();
                    self.stages.zero_drag_bodies+=1;
                }
                cell::advect(''')
    step=replace(step,'                    |position| self.environment.velocity_at(position),','''
                    |position| {
                        if profiling { samples.set(samples.get()+1); }
                        self.environment.velocity_at(position)
                    },''')
    step=replace(step,'''                );
            }
            if let Some(fluid) = &mut self.local_fluid {''','''                );
                self.stages.analytic_samples+=samples.get();
            }
'''+end('advection')+start()+'''if let Some(fluid) = &mut self.local_fluid {''')
    step=replace(step,'            self.physics.step();',end('fluid')+start()+'self.physics.step();\n'+end('rapier'))
    text=text[:a]+step+text[b:]+STAGES
    p.write_text(text)
    p=source/'view.rs'
    p.write_text(replace(p.read_text(),'    pub solve_ms: f64,','    pub solve_ms: f64,\n    pub stages: crate::runtime::StageProfile,'))
    if variant=='skip':
        p=source/'cell.rs'
        text=replace(p.read_text(),'    let response = 1.0 - (-drag * dt).exp();','''
    if drag == 0.0 {
        // Keep Rapier setter bookkeeping (modified flag / kinematic target),
        // while avoiding the expensive analytic velocity sampling.
        for i in 0..body.num_particles() {
            let velocity=body.particle_velocity(i);
            body.set_particle_velocity(i,velocity);
        }
        return;
    }
    let response = 1.0 - (-drag * dt).exp();''')
        p.write_text(text)
    shutil.copy2(ROOT/'probe.rs',folder/'crates/simulation/examples/runtime_probe.rs')


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--seconds',type=float,default=30)
    parser.add_argument('--out',type=Path,default=ROOT/'results')
    args=parser.parse_args()
    args.out=args.out.resolve()
    args.out.mkdir(parents=True,exist_ok=True)
    if (args.out/'manifest.json').exists():
        raise SystemExit('Archived results exist; use a new --out directory.')
    assert args.seconds>0 and {2,3,4,5}<=os.sched_getaffinity(0)
    work=Path(tempfile.mkdtemp(prefix='blood-runtime-hotpaths-',dir='/tmp/sunyunbo'))
    repo=ROOT.parents[3]
    sources=[ROOT/'run.py',ROOT/'probe.rs']
    sources += [SPARSE/f for f in ['run.py','longrun.py','longrun.rs','sparse.rs','compare.rs']]
    sources += [p for p in (SPARSE/'baseline').rglob('*') if p.is_file()]
    manifest={'sources_sha256':{str(p.relative_to(repo)):sha(p) for p in sources},
        'seconds':args.seconds,'scene':'72 cells; sparse Stokes; 1e-4; fixed fork; default input; two substeps; no warmup removal',
        'temporary_workspaces':str(work),'cpus':[2,3,4,5],
        'rustc':subprocess.check_output(['rustc','--version'],text=True).strip(),
        'cpu_model':next(s.split(':',1)[1].strip() for s in Path('/proc/cpuinfo').read_text().splitlines() if s.startswith('model name')),
        'commands':[],'runs':[]}
    binaries={}
    for variant in ['baseline','skip']:
        folder=work/variant
        prepare(folder,variant)
        common=['--release','--offline','--locked','--manifest-path',str(folder/'Cargo.toml'),'-p','simulation','--target-dir',str(TARGET)]
        for name,command in [('cell-test',['cargo','test',*common,'--lib','cell::tests::compressed_cell_preserves_area_and_recovers_after_release']),
                             ('build',['cargo','build',*common,'--example','runtime_probe'])]:
            print(variant,name,flush=True)
            manifest['commands'].append(command)
            with (args.out/f'{variant}-{name}.log').open('w') as log:
                subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
        binaries[variant]=work/f'{variant}-probe'
        shutil.copy2(TARGET/'release/examples/runtime_probe',binaries[variant])
    manifest['binary_sha256']={k:sha(v) for k,v in binaries.items()}
    cases=[('baseline-r1','baseline',False),('skip-r1','skip',False),
           ('skip-r2','skip',False),('baseline-r2','baseline',False),
           ('profile-baseline','baseline',True),('profile-skip','skip',True)]
    (args.out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    for name,variant,profiling in cases:
        out=args.out/name
        out.mkdir()
        env=dict(os.environ)
        env.pop('SPARSE_PROFILE',None)
        env.pop('RUNTIME_PROFILE',None)
        if profiling:env['RUNTIME_PROFILE']='1'
        command=['flock','/tmp/blood-stokes-feedback-bench.lock','taskset','-c','2-5',str(binaries[variant]),str(args.seconds),str(out)]
        print('Running',name,flush=True)
        with (out/'run.log').open('w') as stdout,(out/'stderr.log').open('w') as stderr:
            p=subprocess.run(command,stdout=stdout,stderr=stderr,env=env)
        manifest['runs'].append({'name':name,'variant':variant,'profiling':profiling,'command':command,'exit_code':p.returncode})
        (args.out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
        print(name,'exit',p.returncode,flush=True)
        if p.returncode:raise SystemExit(p.returncode)
    assert all(sha(repo/p)==h for p,h in manifest['sources_sha256'].items())
    print('Completed',args.out,flush=True)


if __name__=='__main__':
    main()
