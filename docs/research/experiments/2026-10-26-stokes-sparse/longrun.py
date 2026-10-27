#!/usr/bin/env python3
"""120-second matched tolerance study; archives diagnostics, no product edits."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

from run import ROOT, TARGET, prepare, sha


LONG_STATS = '''
#[derive(Clone, Debug, Default)]
pub struct LongDiagnostics {
    pub in_volume: f64,
    pub out_volume: f64,
    pub abs_net_volume: f64,
    pub upper_volume: f64,
    pub lower_volume: f64,
    pub flux_relative_peak: f64,
    pub grid_speed_peak: f64,
    pub force_peak: f64,
    pub point_divergence_peak: f64,
    pub divergence_peak: f64,
    pub momentum_peak: f64,
}
'''

GRID_DIAGNOSTICS = '''
    pub fn long_diagnostics(&self, field: &Field) -> (f64,f64,f64,f64,f64,f64) {
        let (mut incoming,mut outgoing,mut upper,mut lower)=(0.0,0.0,0.0,0.0);
        let mut offset=0;
        for c in &self.components {
            for (i,f) in c.faces.iter().enumerate() {
                let q=if f.minus==NONE {-field.velocity[offset+i]*self.h}
                    else if f.plus==NONE {field.velocity[offset+i]*self.h} else {continue;};
                incoming+=(-q).max(0.0);
                outgoing+=q.max(0.0);
                // Three openings: left inlet, upper and lower outlet. An outlet
                // may touch both right and top/bottom sides; group by position.
                if f.position[0] > self.origin[0]+1e-9 {
                    if f.position[1] < 0.0 {upper+=q;} else {lower+=q;}
                }
            }
            offset+=c.faces.len();
        }
        let speed=field.velocity.iter().map(|v|v.abs()).fold(0.0_f64,f64::max);
        let div=self.divergence(&field.velocity).iter().map(|v|v.abs()).fold(0.0_f64,f64::max);
        (incoming,outgoing,upper,lower,speed,div)
    }
'''

UPDATE = '''
        assert!(self.combined.velocity.iter().chain(&self.combined.pressure).all(|x|x.is_finite()),"nonfinite fluid field");
        let (incoming,outgoing,upper,lower,speed,point_div)=self.grid.long_diagnostics(&self.combined);
        let l=&mut self.stats.long;
        l.in_volume+=incoming*dt as f64;
        l.out_volume+=outgoing*dt as f64;
        l.abs_net_volume+=(outgoing-incoming).abs()*dt as f64;
        l.upper_volume+=upper*dt as f64;
        l.lower_volume+=lower*dt as f64;
        if incoming+outgoing > 0.0 {
            l.flux_relative_peak=l.flux_relative_peak.max((outgoing-incoming).abs()/((incoming+outgoing)*0.5));
        }
        l.grid_speed_peak=l.grid_speed_peak.max(speed);
        l.point_divergence_peak=l.point_divergence_peak.max(point_div);
        l.divergence_peak=l.divergence_peak.max(divergence);
        l.momentum_peak=l.momentum_peak.max(momentum);
        for m in &markers {
            let force=m.force[0].hypot(m.force[1]);
            assert!(force.is_finite(),"nonfinite marker force");
            l.force_peak=l.force_peak.max(force);
        }
'''


def prepare_long(folder, tolerance):
    prepare(folder,'sparse')
    fluid=folder/'crates/simulation/src/fluid'
    p=fluid/'grid.rs'
    text=p.read_text()
    assert text.count('let target = 2e-9 *')==1
    text=text.replace('let target = 2e-9 *',f'let target = {tolerance} *')
    text=text.replace('    pub fn boundary_rhs(',GRID_DIAGNOSTICS+'\n    pub fn boundary_rhs(')
    p.write_text(text)
    p=fluid/'mod.rs'
    text=p.read_text()
    text=text.replace('    pub pressure_iterations: usize,','    pub pressure_iterations: usize,\n    pub long: LongDiagnostics,')
    text=text.replace('        self.stats.solve_ms = 0.0;','        self.stats.long=LongDiagnostics::default();\n        self.stats.solve_ms = 0.0;')
    text=text.replace('        let mut body_impulse = [0.0; 2];',UPDATE+'\n        let mut body_impulse = [0.0; 2];')
    p.write_text(text+LONG_STATS)
    shutil.copy2(ROOT/'longrun.rs',folder/'crates/simulation/examples/longrun.rs')


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--seconds',type=float,default=120)
    parser.add_argument('--out',type=Path,default=ROOT/'results/longrun')
    args=parser.parse_args()
    assert args.seconds>0
    args.out=args.out.resolve()
    args.out.mkdir(parents=True,exist_ok=True)
    if (args.out/'manifest.json').exists():
        raise SystemExit('Archived measurements exist; use a new --out directory.')
    work=Path(tempfile.mkdtemp(prefix='blood-stokes-longrun-',dir='/tmp/sunyunbo'))
    sources=[ROOT/f for f in ['run.py','sparse.rs','compare.rs','longrun.py','longrun.rs']]
    sources += [p for p in (ROOT/'baseline').rglob('*') if p.is_file()]
    manifest={'sources_sha256':{str(p.relative_to(ROOT)):sha(p) for p in sources},
              'seconds':args.seconds,'cpus':[2,3,4,5],'temporary_workspaces':str(work),
              'scene':'72 cells; fixed fork; default pulse; no input; 2 substeps; per-frame diagnostics, one-second samples',
              'commands':[],'runs':[]}
    assert {2,3,4,5}<=os.sched_getaffinity(0)
    binaries={}
    for label,tol in [('strict',2e-9),('loose',1e-4)]:
        folder=work/label
        prepare_long(folder,tol)
        command=['cargo','build','--release','--offline','--locked','--manifest-path',str(folder/'Cargo.toml'),
                 '-p','simulation','--example','longrun','--target-dir',str(TARGET)]
        print('Building',label,flush=True)
        manifest['commands'].append(command)
        with (args.out/f'{label}-build.log').open('w') as stream:
            subprocess.run(command,stdout=stream,stderr=subprocess.STDOUT,check=True)
        binaries[label]=work/f'{label}-longrun'
        shutil.copy2(TARGET/'release/examples/longrun',binaries[label])
    manifest['binary_sha256']={k:sha(v) for k,v in binaries.items()}
    (args.out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    for label in ['loose','strict']:
        out=args.out/label
        out.mkdir()
        env=dict(os.environ)
        env.pop('SPARSE_PROFILE',None)
        command=['flock','/tmp/blood-stokes-feedback-bench.lock','taskset','-c','2-5',
                 str(binaries[label]),str(args.seconds),str(out)]
        print('Running',label,flush=True)
        with (out/'run.log').open('w') as stdout,(out/'stderr.log').open('w') as stderr:
            p=subprocess.run(command,stdout=stdout,stderr=stderr,env=env)
        manifest['runs'].append({'name':label,'tolerance':1e-4 if label=='loose' else 2e-9,
                                 'command':command,'exit_code':p.returncode})
        (args.out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
        print(label,'exit',p.returncode,flush=True)
        if p.returncode:
            print((out/'stderr.log').read_text()[-1500:],flush=True)
    assert all(sha(ROOT/p)==h for p,h in manifest['sources_sha256'].items())
    print('Completed',args.out,flush=True)


if __name__=='__main__':
    main()
