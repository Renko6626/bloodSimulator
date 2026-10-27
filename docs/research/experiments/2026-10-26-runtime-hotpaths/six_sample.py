#!/usr/bin/env python3
"""User-requested six flow samples, interpolated to the original 12 particles."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parent
SPEC=importlib.util.spec_from_file_location('runtime_trial',ROOT/'run.py')
TRIAL=importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TRIAL)

OLD='''    let response = 1.0 - (-drag * dt).exp();
    for i in 0..body.num_particles() {
        let target = velocity_at(body.particle_position(i));
        let velocity = body.particle_velocity(i);
        body.set_particle_velocity(i, velocity + (target - velocity) * response);
    }'''
NEW='''    let response = 1.0 - (-drag * dt).exp();
    assert_eq!(body.num_particles(),12,"six-sample research case uses a 12-node ring");
    let samples: [Vector;6]=std::array::from_fn(|k| velocity_at(body.particle_position(k*2)));
    for i in 0..12 {
        let k=i/2;
        let target=if i%2==0 {samples[k]} else {(samples[k]+samples[(k+1)%6])*0.5};
        let velocity=body.particle_velocity(i);
        body.set_particle_velocity(i,velocity+(target-velocity)*response);
    }'''

CHECK='''
#[cfg(test)]
mod uniform_drag_check {
    use super::*;
    #[test]
    fn uniform_flow_keeps_full_ring_response() {
        let mut world=PhysicsWorld::new();
        let config=Config::default();
        let handle=insert(&mut world,Vector::ZERO,&config);
        let body=&mut world.soft_bodies[handle];
        for i in 0..12 {
            body.set_particle_velocity(i,Vector::new(i as f32*0.03,-(i as f32)*0.02));
        }
        let before: Vec<_>=body.particle_velocities().collect();
        let target=Vector::new(1.25,-0.5);
        let dt=1.0/120.0;
        advect(body,20.0,dt,|_|target);
        assert_eq!(body.num_particles(),12);
        let decay=(-20.0_f64*(dt as f64)).exp();
        for i in 0..12 {
            let expected=Vector::new(
                (target.x as f64+(before[i].x-target.x) as f64*decay) as f32,
                (target.y as f64+(before[i].y-target.y) as f64*decay) as f32);
            assert!((body.particle_velocity(i)-expected).length()<1e-6);
        }
    }
}
'''


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--seconds',type=float,default=30)
    parser.add_argument('--out',type=Path,default=ROOT/'results/six-sample')
    args=parser.parse_args()
    assert args.seconds>0 and {2,3,4,5}<=os.sched_getaffinity(0)
    args.out=args.out.resolve()
    args.out.mkdir(parents=True,exist_ok=True)
    if (args.out/'manifest.json').exists():
        raise SystemExit('Archived results exist; use a new --out directory.')
    repo=ROOT.parents[3]
    original=json.loads((ROOT/'results/manifest.json').read_text())
    inputs=dict(original['sources_sha256'])
    inputs[str(Path(__file__).resolve().relative_to(repo))]=TRIAL.sha(Path(__file__))
    assert all(TRIAL.sha(repo/p)==h for p,h in inputs.items())
    work=Path(tempfile.mkdtemp(prefix='blood-six-sample-',dir='/tmp/sunyunbo'))
    manifest={'sources_sha256':inputs,'seconds':args.seconds,'cpus':[2,3,4,5],
        'temporary_workspaces':str(work),
        'scene':'72 cells; 12 physical nodes; sparse Stokes 1e-4; four fluid markers; both skip zero drag; 12 vs 6 analytic flow samples',
        'commands':[],'runs':[]}
    binaries={}
    for variant in ['twelve','six']:
        folder=work/variant
        TRIAL.prepare(folder,'skip')
        p=folder/'crates/simulation/src/cell.rs'
        text=p.read_text()
        if variant=='six':
            assert text.count(OLD)==1
            text=text.replace(OLD,NEW)
        p.write_text(text+CHECK)
        common=['--release','--offline','--locked','--manifest-path',str(folder/'Cargo.toml'),'-p','simulation','--target-dir',str(TRIAL.TARGET)]
        for name,command in [('checks',['cargo','test',*common,'--lib','cell::']),
                             ('build',['cargo','build',*common,'--example','runtime_probe'])]:
            print(variant,name,flush=True)
            manifest['commands'].append(command)
            with (args.out/f'{variant}-{name}.log').open('w') as log:
                subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
        binaries[variant]=work/f'{variant}-probe'
        shutil.copy2(TRIAL.TARGET/'release/examples/runtime_probe',binaries[variant])
    manifest['binary_sha256']={k:TRIAL.sha(v) for k,v in binaries.items()}
    for name,variant,profiling in [('twelve-r1','twelve',False),('six-r1','six',False),
            ('six-r2','six',False),('twelve-r2','twelve',False),
            ('profile-twelve','twelve',True),('profile-six','six',True)]:
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
    assert all(TRIAL.sha(repo/p)==h for p,h in inputs.items())
    print('Completed',args.out,flush=True)


if __name__=='__main__':
    main()
