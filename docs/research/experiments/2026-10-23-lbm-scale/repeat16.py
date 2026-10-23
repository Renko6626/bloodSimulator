"""Two extra reversed-order pairs to inspect the noisy 16-cell timing result."""
import csv
import hashlib
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent
meta = json.loads((ROOT/'results/environment.json').read_text())
for name,digest in meta['sha256'].items():
    assert hashlib.sha256((ROOT/name).read_bytes()).hexdigest()==digest,name
out = ROOT/'results/repeat16'
out.mkdir(exist_ok=True)
schedule = [('reverse','cached'),('reverse','baseline'),('forward','baseline'),('forward','cached')]
(out/'order.json').write_text(json.dumps({'cpu':meta['cpu'],'seconds':meta['seconds'],
                                         'order':schedule,'exclusive':False},indent=2))
env = os.environ.copy()
env['CARGO_TARGET_DIR'] = str(ROOT.parents[3]/'target')
subprocess.run(['cargo','build','--manifest-path',str(ROOT/'rapier/Cargo.toml'),
                '--offline','--locked','--release'],env=env,check=True)
binary = str(Path(env['CARGO_TARGET_DIR'])/'release/lbm-rapier-scale-probe')
for pair,mode in schedule:
    dest = out/f'{pair}-{mode}'
    print(f'Repeat 16 cells: {pair}, {mode}',flush=True)
    result = subprocess.check_output(['taskset','-c',str(meta['cpu']),binary,'--outdir',str(dest),
                                      '--mode',mode,'--scene','scale','--count','16',
                                      '--duration',str(meta['seconds'])],text=True)
    (dest/'run.log').write_text(result)
    for filename in ('frames.csv','outlines.csv'):
        base = list(csv.DictReader((ROOT/f'results/scale-16-{mode}'/filename).open()))
        trial = list(csv.DictReader((dest/filename).open()))
        assert len(base)==len(trial)
        for a,b in zip(base,trial):
            for key,val in a.items():
                if not key.endswith('_ms'):
                    assert val==b[key],(pair,mode,filename,key)
    # The duplicate outlines are numerically identical to the initial archive.
    (dest/'outlines.csv').unlink()
print('All repeated trajectories reproduce the initial non-timing outputs.',flush=True)
