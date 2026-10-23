"""Serial, paired kernel-reuse and multi-cell research runs."""
import argparse
import csv
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--cpu', type=int, default=2)
parser.add_argument('--seconds', type=float, default=6)
args = parser.parse_args()
assert args.cpu in os.sched_getaffinity(0) and args.seconds >= 3
out = ROOT/'results'
out.mkdir(exist_ok=True)
sources = ['shared/fluid.rs','shared/check_prepared.rs','rapier/src/main.rs',
           'rapier/Cargo.toml','rapier/Cargo.lock']
meta = dict(utc=datetime.datetime.now(datetime.timezone.utc).isoformat(), cpu=args.cpu,
            seconds=args.seconds, exclusive=False,
            rustc=subprocess.check_output(['rustc','--version'],text=True).strip(),
            lscpu=subprocess.check_output(['lscpu'],text=True),
            sha256={n:hashlib.sha256((ROOT/n).read_bytes()).hexdigest() for n in sources})
(out/'environment.json').write_text(json.dumps(meta,indent=2))
with tempfile.TemporaryDirectory(prefix='lbm-prepared-check-') as tmp:
    binary = str(Path(tmp)/'check')
    subprocess.run(['rustc','--edition','2021','-O','-Awarnings',
                    str(ROOT/'shared/check_prepared.rs'),'-o',binary],check=True)
    result = subprocess.check_output([binary],text=True)
    (out/'kernel-equivalence.log').write_text(result)
    print(result.strip(),flush=True)
env = os.environ.copy()
env['CARGO_TARGET_DIR'] = str(ROOT.parents[3]/'target')
subprocess.run(['cargo','build','--manifest-path',str(ROOT/'rapier/Cargo.toml'),
                '--offline','--locked','--release'],env=env,check=True)
binary = str(Path(env['CARGO_TARGET_DIR'])/'release/lbm-rapier-scale-probe')
pairs = [('reversal',1),('squeeze',1),('scale',1),('scale',4),('scale',16)]
for scene,count in pairs:
    for mode in ('baseline','cached'):
        dest = out/f'{scene}-{count}-{mode}'
        cmd = ['taskset','-c',str(args.cpu),binary,'--outdir',str(dest),'--mode',mode,
               '--scene',scene,'--count',str(count),'--duration',str(args.seconds)]
        print(f'Run {scene}, count={count}, mode={mode}',flush=True)
        result = subprocess.check_output(cmd,text=True)
        (dest/'run.log').write_text(result)
    for filename in ('frames.csv','outlines.csv'):
        a = list(csv.DictReader((out/f'{scene}-{count}-baseline'/filename).open()))
        b = list(csv.DictReader((out/f'{scene}-{count}-cached'/filename).open()))
        assert len(a) == len(b)
        for left,right in zip(a,b):
            for key,value in left.items():
                if key != 'mode' and not key.endswith('_ms'):
                    assert value == right[key],(scene,count,filename,key,value,right[key])
    print('  All non-timing output fields match exactly.',flush=True)
dest = out/'counterflow-16-cached'
result = subprocess.check_output(['taskset','-c',str(args.cpu),binary,'--outdir',str(dest),
                                  '--mode','cached','--scene','counterflow','--count','16',
                                  '--duration',str(args.seconds)],text=True)
(dest/'run.log').write_text(result)
print(result.strip(),flush=True)
print('Paired and contact-input research runs complete.',flush=True)
