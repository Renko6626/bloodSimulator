"""Run the same single-cell harness with f32 fluid storage/arithmetic only."""
import argparse
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
out = ROOT/'results/f32'
out.mkdir(exist_ok=True)
files = ['shared/fluid_f32.rs', 'rapier/src/main.rs', 'rapier/Cargo.toml', 'rapier/Cargo.lock']
meta = dict(utc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
            cpu=args.cpu, seconds=args.seconds, exclusive=False,
            rustc=subprocess.check_output(['rustc','--version'],text=True).strip(),
            sha256={name:hashlib.sha256((ROOT/name).read_bytes()).hexdigest() for name in files},
            change='Only the fluid module import is replaced; Rapier/scenarios/checks are identical.')
(out/'environment.json').write_text(json.dumps(meta,indent=2))
with tempfile.TemporaryDirectory(prefix='lbm-rapier-f32-') as tmp:
    package = Path(tmp)
    (package/'src').mkdir()
    old_name = 'lbm-rapier-dynamics-probe'
    new_name = 'lbm-rapier-dynamics-f32-probe'
    for name in ('Cargo.toml','Cargo.lock'):
        (package/name).write_text((ROOT/'rapier'/name).read_text().replace(old_name,new_name))
    source = (ROOT/'rapier/src/main.rs').read_text()
    source = source.replace('"../../shared/fluid.rs"', json.dumps(str(ROOT/'shared/fluid_f32.rs')))
    (package/'src/main.rs').write_text(source)
    env = os.environ.copy()
    env['CARGO_TARGET_DIR'] = str(ROOT.parents[3]/'target')
    subprocess.run(['cargo','build','--manifest-path',str(package/'Cargo.toml'),
                    '--offline','--locked','--release'],env=env,check=True)
    binary = str(Path(env['CARGO_TARGET_DIR'])/'release'/new_name)
    text = subprocess.check_output(['taskset','-c',str(args.cpu),binary,str(out),str(args.seconds)],text=True)
    (out/'run.log').write_text(text)
    print(text.strip(),flush=True)
print('f32 single-cell comparison complete; original acceptance checks unchanged.',flush=True)
