#!/usr/bin/env python3
"""Further profiling of accepted six-sample candidate; no algorithm changes."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib

ROOT=Path(__file__).resolve().parent
PREVIOUS=ROOT.parent/'2026-10-26-runtime-hotpaths'
REPO=ROOT.parents[3]
SPEC=importlib.util.spec_from_file_location('six_trial',PREVIOUS/'six_sample.py')
SIX=importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SIX)
TRIAL=SIX.TRIAL


def replace(text,old,new):
    assert text.count(old)==1,old[:120]
    return text.replace(old,new)


RAPIER='''
            if crate::profile::mode()==3 {
                let c=&self.physics.physics_pipeline.counters;
                for (id,ns) in [
                    (30,c.stages.update_time.time().as_nanos()),
                    (31,c.stages.collision_detection_time.time().as_nanos()),
                    (32,c.stages.island_construction_time.time().as_nanos()),
                    (33,c.stages.island_constraints_collection_time.time().as_nanos()),
                    (34,c.stages.solver_time.time().as_nanos()),
                    (35,c.stages.user_changes.time().as_nanos()),
                    (36,c.stages.ccd_time.time().as_nanos()),
                    (37,c.cd.broad_phase_time.time().as_nanos()),
                    (38,c.cd.narrow_phase_time.time().as_nanos()),
                    (39,c.cd.final_broad_phase_time.time().as_nanos()),
                    (40,c.solver.velocity_assembly_time.time().as_nanos()),
                    (41,c.solver.velocity_resolution_time.time().as_nanos()),
                    (42,c.solver.velocity_writeback_time.time().as_nanos())
                ] { crate::profile::add(id,ns as u64); }
            }
'''


def prepare(folder):
    TRIAL.prepare(folder,'skip')
    src=folder/'crates/simulation/src'
    p=src/'cell.rs'
    p.write_text(replace(p.read_text(),SIX.OLD,SIX.NEW))
    shutil.copy2(ROOT/'profile.rs',src/'profile.rs')
    p=src/'lib.rs'
    p.write_text(p.read_text()+'\npub mod profile;\n')
    p=src/'view.rs'
    p.write_text(replace(p.read_text(),'    pub solve_ms: f64,','    pub solve_ms: f64,\n    pub deep: crate::profile::DeepStats,'))
    p=src/'runtime.rs'
    text=p.read_text()
    text=replace(text,'    stages: StageProfile,','    stages: StageProfile,\n    deep: crate::profile::DeepStats,')
    text=replace(text,'            stages: StageProfile::default(),','            stages: StageProfile::default(),\n            deep: crate::profile::DeepStats::default(),')
    text=replace(text,'            stages: self.stages.clone(),','            stages: self.stages.clone(),\n            deep: self.deep.clone(),')
    text=replace(text,'    fn step_inner(&mut self, input: MoveInput) {','''    fn step_inner(&mut self, input: MoveInput) {
        crate::profile::begin();
        if crate::profile::mode()==3 { self.physics.physics_pipeline.counters.enable(); }''')
    text=replace(text,'self.physics.step();','self.physics.step();\n'+RAPIER)
    text=replace(text,'        self.steps += 1;','        self.deep=crate::profile::finish();\n        self.steps += 1;')
    p.write_text(text)
    p=src/'circulation/graph.rs'
    text=replace(p.read_text(),'''pub fn sample_velocity(position: Vector, map: &MapDefinition, flow: &GraphFlow) -> Vector {''','''pub fn sample_velocity(position: Vector, map: &MapDefinition, flow: &GraphFlow) -> Vector {
    let _total=crate::profile::span(0);''')
    text=replace(text,'    let mut best_junction = None;','    let search=crate::profile::span(5);\n    let mut best_junction = None;')
    text=replace(text,'    if let Some((node_id, _, radius)) = best_junction {','    drop(search);\n    if let Some((node_id, _, radius)) = best_junction {\n        let _mix=crate::profile::span(6);')
    p.write_text(text)
    p=src/'map.rs'
    text=p.read_text()
    for name,id,signature in [
        ('nearest_edge',1,'pub fn nearest_edge(&self, position: Vector) -> usize {'),
        ('signed_distance',2,'pub fn signed_distance(&self, position: Vector) -> f32 {'),
        ('project_edge',3,'pub fn project_edge(&self, edge: usize, position: Vector) -> EdgeProjection {'),
        ('edge_length',4,'pub fn edge_length(&self, edge: usize) -> f32 {')]:
        text=replace(text,signature,signature+f'\nlet _profile=crate::profile::span({id});')
    p.write_text(text)
    p=src/'fluid/mod.rs'
    text=p.read_text()
    text=replace(text,'        let mut markers = Vec::new();','        let _all=crate::profile::span(10);\n        let markers_time=crate::profile::span(11);\n        let mut markers = Vec::new();')
    text=replace(text,'        let mut rhs = vec![0.0; self.grid.velocity_count()];','        drop(markers_time);\n        let scatter_time=crate::profile::span(12);\n        let mut rhs = vec![0.0; self.grid.velocity_count()];')
    text=replace(text,'        let feedback = self','        drop(scatter_time);\n        let feedback = self')
    text=replace(text,'        for (out, (base, delta)) in self\n            .combined\n            .velocity','        let combine_time=crate::profile::span(14);\n        for (out, (base, delta)) in self\n            .combined\n            .velocity')
    text=replace(text,'        let full_rhs: Vec<_> = rhs','        drop(combine_time);\n        let diagnostic_time=crate::profile::span(15);\n        let full_rhs: Vec<_> = rhs')
    text=replace(text,'        let mut body_impulse = [0.0; 2];','        drop(diagnostic_time);\n        let _impulses=crate::profile::span(16);\n        let mut body_impulse = [0.0; 2];')
    p.write_text(text)
    p=src/'fluid/grid.rs'
    text=p.read_text()
    for signature,id in [
        ('fn inverse(&self, rhs: &[f64]) -> Vec<f64> {',21),
        ('fn divergence(&self, velocity: &[f64]) -> Vec<f64> {',18),
        ('fn transpose(&self, pressure: &[f64]) -> Vec<f64> {',17),
        ('fn schur(&self, pressure: &[f64]) -> Vec<f64> {',19),
        ('pub fn measure(&self, velocity: &[f64], pressure: &[f64], rhs: &[f64]) -> (f64, f64, f64) {',20)]:
        text=replace(text,signature,signature+f'\nlet _profile=crate::profile::span({id});')
    text=replace(text,'        assert_eq!(force.len(), self.velocity_count());','        let _grid_solve=crate::profile::span(13);\n        assert_eq!(force.len(), self.velocity_count());')
    text=replace(text,'        let (mut result, other) = crate::threading::install(|| {','        let ((mut result,ut), (other,vt)) = crate::threading::install(|| {')
    text=replace(text,'''                || self.components[0].factor.solve(&rhs[..split]),
                || self.components[1].factor.solve(&rhs[split..]),''','''
                || { let t=(crate::profile::mode()==2).then(std::time::Instant::now); let x=self.components[0].factor.solve(&rhs[..split]); (x,t.map(|t|t.elapsed().as_nanos() as u64)) },
                || { let t=(crate::profile::mode()==2).then(std::time::Instant::now); let x=self.components[1].factor.solve(&rhs[split..]); (x,t.map(|t|t.elapsed().as_nanos() as u64)) },''')
    text=replace(text,'        result.extend(other);','''
        if let (Some(u),Some(v))=(ut,vt) {
            crate::profile::add(27,u); crate::profile::add(28,v); crate::profile::add(29,u.max(v));
        }
        result.extend(other);''')
    p.write_text(text)
    p=src/'fluid/linear.rs'
    text=p.read_text()
    text=replace(text,'        assert_eq!(rhs.len(), self.order.len());','        let _all=crate::profile::span(22);\n        let reorder=crate::profile::span(23);\n        assert_eq!(rhs.len(), self.order.len());')
    text=replace(text,'        for j in 0..x.len() {','        drop(reorder);\n        let forward=crate::profile::span(24);\n        for j in 0..x.len() {')
    text=replace(text,'        for j in (0..x.len()).rev() {','        drop(forward);\n        let backward=crate::profile::span(25);\n        for j in (0..x.len()).rev() {')
    text=replace(text,'        let mut result = vec![0.0; x.len()];','        drop(backward);\n        let _restore=crate::profile::span(26);\n        let mut result = vec![0.0; x.len()];')
    p.write_text(text)
    p=folder/'crates/simulation/examples/runtime_probe.rs'
    text=p.read_text()
    text=replace(text,'    let mut stages = BufWriter::new','''    let mut deep=BufWriter::new(std::fs::File::create(out.join("deep.csv")).unwrap());
    write!(deep,"frame").unwrap();
    for name in simulation::profile::NAMES { write!(deep,",{name}_ns,{name}_calls").unwrap(); }
    writeln!(deep).unwrap();
    let mut stages = BufWriter::new''')
    text=replace(text,'        let p = &state.stages;','''        write!(deep,"{step}").unwrap();
        for i in 0..simulation::profile::N { write!(deep,",{},{}",state.deep.ns[i],state.deep.calls[i]).unwrap(); }
        writeln!(deep).unwrap();
        let p = &state.stages;''')
    text=replace(text,'            stages.flush().unwrap();','            deep.flush().unwrap();\n            stages.flush().unwrap();')
    p.write_text(text)


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--seconds',type=float,default=30)
    parser.add_argument('--out',type=Path,default=ROOT/'results')
    args=parser.parse_args()
    assert args.seconds>0 and {2,3,4,5}<=os.sched_getaffinity(0)
    args.out=args.out.resolve();args.out.mkdir(parents=True,exist_ok=True)
    if (args.out/'manifest.json').exists():raise SystemExit('Use a new --out; archived results exist.')
    sources=dict(json.loads((PREVIOUS/'results/six-sample/manifest.json').read_text())['sources_sha256'])
    for name in ['run.py','profile.rs']:
        p=ROOT/name;sources[str(p.relative_to(REPO))]=TRIAL.sha(p)
    assert all(TRIAL.sha(REPO/p)==h for p,h in sources.items())
    work=Path(tempfile.mkdtemp(prefix='blood-six-profile-',dir='/tmp/sunyunbo'))
    folder=work/'workspace';prepare(folder)
    common=['--release','--offline','--manifest-path',str(folder/'Cargo.toml'),'-p','simulation','--target-dir',str(TRIAL.TARGET)]
    command=['cargo','build',*common,'--locked','--example','runtime_probe']
    print('Building deep profile',flush=True)
    with (args.out/'build.log').open('w') as log:subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
    plain=work/'plain-probe';shutil.copy2(TRIAL.TARGET/'release/examples/runtime_probe',plain)
    # Activate only Rapier's existing profiler feature. Preserve original package
    # versions and freeze the diagnostic lock separately; never edit repo locks.
    before=tomllib.loads((folder/'Cargo.lock').read_text())
    metadata=['cargo','metadata','--offline','--format-version','1','--manifest-path',str(folder/'Cargo.toml'),'--features','rapier2d/profiler']
    print('Preparing native Rapier counters',flush=True)
    with (args.out/'metadata.log').open('w') as log:subprocess.run(metadata,stdout=log,stderr=subprocess.STDOUT,check=True)
    after=tomllib.loads((folder/'Cargo.lock').read_text())
    old={(p['name'],p['version']) for p in before['package']}
    new={(p['name'],p['version']) for p in after['package']}
    assert old<=new,('unexpected package version changes',old-new)
    shutil.copy2(folder/'Cargo.lock',args.out/'Cargo-profiler.lock')
    native_command=['cargo','build',*common,'--locked','--features','rapier2d/profiler','--example','runtime_probe']
    with (args.out/'native-build.log').open('w') as log:subprocess.run(native_command,stdout=log,stderr=subprocess.STDOUT,check=True)
    native=work/'native-probe';shutil.copy2(TRIAL.TARGET/'release/examples/runtime_probe',native)
    manifest={'sources_sha256':sources,'seconds':args.seconds,'cpus':[2,3,4,5],
        'temporary_workspaces':str(work),'commands':[command,metadata,native_command],
        'additional_diagnostic_packages':sorted(new-old),
        'binary_sha256':{'plain':TRIAL.sha(plain),'native':TRIAL.sha(native)},'runs':[],
        'scene':'accepted six analytic samples; 12 soft particles; 4 fluid markers; sparse Stokes 1e-4; 72 cells; fixed fork; 2 substeps; default input',
        'perf':'unavailable: perf_event_paranoid=4; perf stat -e task-clock true denied; no sysctl change'}
    for name,mode,binary in [('control-r1','none',plain),('geometry','geometry',plain),('fluid','fluid',plain),('rapier','rapier',native),('control-r2','none',plain)]:
        out=args.out/name;out.mkdir()
        env=dict(os.environ);env.pop('SPARSE_PROFILE',None)
        env['RUNTIME_PROFILE']='1';env['DEEP_MODE']=mode
        cmd=['flock','/tmp/blood-stokes-feedback-bench.lock','taskset','-c','2-5',str(binary),str(args.seconds),str(out)]
        print('Running',name,flush=True)
        with (out/'run.log').open('w') as stdout,(out/'stderr.log').open('w') as stderr:
            p=subprocess.run(cmd,stdout=stdout,stderr=stderr,env=env)
        manifest['runs'].append({'name':name,'mode':mode,'command':cmd,'exit_code':p.returncode})
        (args.out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
        print(name,'exit',p.returncode,flush=True)
        if p.returncode:raise SystemExit(p.returncode)
    assert all(TRIAL.sha(REPO/p)==h for p,h in sources.items())
    print('Completed',args.out,flush=True)


if __name__=='__main__':main()
