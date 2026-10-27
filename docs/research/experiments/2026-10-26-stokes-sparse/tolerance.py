#!/usr/bin/env python3
"""Separate, user-requested pressure-tolerance trial on the sparse candidate."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

from run import ROOT, TARGET, prepare, sha


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', type=Path, default=ROOT/'results/tolerance')
    args = parser.parse_args()
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    if (args.out/'manifest.json').exists():
        raise SystemExit('Use --out for new measurements; archived results already exist.')
    work = Path(tempfile.mkdtemp(prefix='blood-stokes-tolerance-',dir='/tmp/sunyunbo'))
    tolerances = {'strict':2e-9,'medium':1e-6,'loose':1e-4}
    sources = [ROOT/'run.py',ROOT/'sparse.rs',ROOT/'tolerance.py',ROOT/'compare.rs']
    sources += [p for p in (ROOT/'baseline').rglob('*') if p.is_file()]
    manifest = {'sources_sha256':{str(p.relative_to(ROOT)):sha(p) for p in sources},
                'tolerances':tolerances,'temporary_workspaces':str(work),
                'scene':'same 72-cell fixed fork; 60 frames; CPU 2-5; sparse minimum-degree Cholesky; no profiling',
                'commands':[],'runs':[]}
    binaries = {}
    for label, tol in tolerances.items():
        folder = work/label
        prepare(folder,'sparse')
        grid = folder/'crates/simulation/src/fluid/grid.rs'
        text = grid.read_text()
        assert text.count('let target = 2e-9 *') == 1
        text = text.replace('let target = 2e-9 *',f'let target = {tol} *')
        text = text.replace('''    pub fn boundary_rhs(''', '''
    pub fn opening_fluxes(&self, field: &Field) -> (f64,f64) {
        let mut incoming = 0.0;
        let mut outgoing = 0.0;
        let mut offset = 0;
        for c in &self.components {
            for (i,f) in c.faces.iter().enumerate() {
                let flux = if f.minus == NONE { -field.velocity[offset+i]*self.h }
                    else if f.plus == NONE { field.velocity[offset+i]*self.h } else { continue; };
                incoming += (-flux).max(0.0);
                outgoing += flux.max(0.0);
            }
            offset += c.faces.len();
        }
        (incoming,outgoing)
    }
    pub fn boundary_rhs(''')
        grid.write_text(text)
        fluid = folder/'crates/simulation/src/fluid/mod.rs'
        text = fluid.read_text().replace('''        let mut body_impulse = [0.0; 2];''','''
        let (incoming,outgoing) = self.grid.opening_fluxes(&self.combined);
        eprintln!("FLUX,{incoming:.17e},{outgoing:.17e}");
        let mut body_impulse = [0.0; 2];''')
        fluid.write_text(text)
        command = ['cargo','build','--release','--offline','--locked','--manifest-path',str(folder/'Cargo.toml'),
                   '-p','simulation','--example','circulation_probe','--target-dir',str(TARGET)]
        print('Building tolerance',label,tol,flush=True)
        manifest['commands'].append(command)
        with (args.out/f'{label}-build.log').open('w') as log:
            subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
        binaries[label] = work/f'{label}-probe'
        shutil.copy2(TARGET/'release/examples/circulation_probe',binaries[label])
    manifest['binary_sha256'] = {k:sha(v) for k,v in binaries.items()}
    for label, repeat in [('strict',1),('medium',1),('loose',1),('loose',2),('medium',2),('strict',2)]:
        name = f'{label}-r{repeat}'
        out = args.out/name
        out.mkdir()
        env = dict(os.environ)
        env.pop('SPARSE_PROFILE',None)
        command = ['flock','/tmp/blood-stokes-feedback-bench.lock','taskset','-c','2-5',
                   str(binaries[label]),'72','1','local',str(out)]
        print('Running',name,flush=True)
        with (out/'run.log').open('w') as stdout, (out/'profile.log').open('w') as stderr:
            p = subprocess.run(command,stdout=stdout,stderr=stderr,env=env)
        manifest['runs'].append({'name':name,'tolerance':tolerances[label],'command':command,'exit_code':p.returncode})
        (args.out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
        if p.returncode:
            raise SystemExit(p.returncode)
    assert all(sha(ROOT/p)==h for p,h in manifest['sources_sha256'].items())
    print('Completed',args.out,flush=True)


if __name__ == '__main__':
    main()
