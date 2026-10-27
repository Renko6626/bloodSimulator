#!/usr/bin/env python3
"""Matched before/after benchmark of the production one-pass geometry query."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parent
REPO=ROOT.parents[3]
PROFILE=ROOT.parent/'2026-10-27-six-profile'
SPEC=importlib.util.spec_from_file_location('deep_trial',PROFILE/'run.py')
DEEP=importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DEEP)
TRIAL=DEEP.TRIAL
FROZEN=ROOT.parent/'2026-10-26-stokes-sparse/baseline/crates/simulation/src'


def instrument(text,signature,id):
    assert text.count(signature)==1,signature
    pos=text.index('{',text.index(signature))+1
    return text[:pos]+f'\nlet _geometry_profile=crate::profile::span({id});'+text[pos:]


def prepare(folder,variant):
    DEEP.prepare(folder)
    src=folder/'crates/simulation/src'
    if variant=='reuse':
        for name in ['map.rs','circulation/graph.rs']:
            shutil.copy2(REPO/'crates/simulation/src'/name,src/name)
        p=src/'map.rs';text=p.read_text()
        for signature,id in [('pub(crate) fn nearest_edge_and_signed_distance(',1),
                ('pub fn signed_distance(',2),('pub fn project_edge(',3),('pub fn edge_length(',4)]:
            text=instrument(text,signature,id)
        p.write_text(text)
        p=src/'circulation/graph.rs'
        text=instrument(p.read_text(),'pub fn sample_velocity(',0)
        text=DEEP.replace(text,'    let mut best_junction = None;','    let search=crate::profile::span(5);\n    let mut best_junction = None;')
        text=DEEP.replace(text,'    if let Some((node_id, _, radius)) = best_junction {','    drop(search);\n    if let Some((node_id, _, radius)) = best_junction {\n        let _mix=crate::profile::span(6);')
        p.write_text(text)
    # The archived reference is actual old production code, not a rewritten
    # mathematical oracle. Only the temporary test build includes it.
    shutil.copy2(FROZEN/'circulation/graph.rs',src/'circulation/geometry_reference.rs')
    p=src/'circulation.rs'
    p.write_text(p.read_text()+'\n#[cfg(test)]\nmod geometry_reference;\n'+(ROOT/'audit.rs').read_text())


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--seconds',type=float,default=30)
    parser.add_argument('--out',type=Path,default=ROOT/'results')
    args=parser.parse_args()
    assert args.seconds>0 and {2,3,4,5}<=os.sched_getaffinity(0)
    args.out=args.out.resolve();args.out.mkdir(parents=True,exist_ok=True)
    if (args.out/'manifest.json').exists():raise SystemExit('Use a new --out; measurements are archived.')
    inputs=dict(json.loads((PROFILE/'results/manifest.json').read_text())['sources_sha256'])
    for p in [ROOT/'run.py',ROOT/'audit.rs',REPO/'crates/simulation/src/map.rs',REPO/'crates/simulation/src/circulation/graph.rs']:
        inputs[str(p.relative_to(REPO))]=TRIAL.sha(p)
    assert all(TRIAL.sha(REPO/p)==h for p,h in inputs.items())
    work=Path(tempfile.mkdtemp(prefix='blood-geometry-query-',dir='/tmp/sunyunbo'))
    manifest={'sources_sha256':inputs,'seconds':args.seconds,'cpus':[2,3,4,5],
        'temporary_workspaces':str(work),'commands':[],'runs':[],
        'scene':'accepted 6 analytic samples, 12 physical nodes, sparse Stokes 1e-4; 72 cells; 2 substeps; fixed fork; default input; only query reuse differs'}
    binaries={}
    for variant in ['before','reuse']:
        folder=work/variant;prepare(folder,variant)
        common=['--release','--offline','--locked','--manifest-path',str(folder/'Cargo.toml'),'-p','simulation','--target-dir',str(TRIAL.TARGET)]
        for label,command in [('audit',['cargo','test',*common,'--lib','geometry_query_audit','--','--nocapture']),
                              ('build',['cargo','build',*common,'--example','runtime_probe'])]:
            print(variant,label,flush=True);manifest['commands'].append(command)
            with (args.out/f'{variant}-{label}.log').open('w') as log:
                subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
        binaries[variant]=work/f'{variant}-probe'
        shutil.copy2(TRIAL.TARGET/'release/examples/runtime_probe',binaries[variant])
    manifest['binary_sha256']={k:TRIAL.sha(v) for k,v in binaries.items()}
    for name,variant,mode in [('before-r1','before','none'),('reuse-r1','reuse','none'),
            ('reuse-r2','reuse','none'),('before-r2','before','none'),
            ('geometry-before','before','geometry'),('geometry-reuse','reuse','geometry')]:
        out=args.out/name;out.mkdir()
        env=dict(os.environ);env.pop('SPARSE_PROFILE',None)
        env['RUNTIME_PROFILE']='1';env['DEEP_MODE']=mode
        command=['flock','/tmp/blood-stokes-feedback-bench.lock','taskset','-c','2-5',str(binaries[variant]),str(args.seconds),str(out)]
        print('Running',name,flush=True)
        with (out/'run.log').open('w') as stdout,(out/'stderr.log').open('w') as stderr:
            p=subprocess.run(command,stdout=stdout,stderr=stderr,env=env)
        manifest['runs'].append({'name':name,'variant':variant,'mode':mode,'command':command,'exit_code':p.returncode})
        (args.out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
        print(name,'exit',p.returncode,flush=True)
        if p.returncode:raise SystemExit(p.returncode)
    assert all(TRIAL.sha(REPO/p)==h for p,h in inputs.items())
    print('Completed',args.out,flush=True)


if __name__=='__main__':main()
