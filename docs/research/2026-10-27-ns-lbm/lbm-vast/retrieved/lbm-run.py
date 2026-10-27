#!/usr/bin/env python3
"""Run the approved first batch on a verified GPU. No cloud lifecycle actions."""
import argparse,datetime,hashlib,json,os,subprocess,time
from pathlib import Path
HERE=Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--binary',type=Path,required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--frames',type=int,default=120);p.add_argument('--cpus',default='2-5');p.add_argument('--software-check',action='store_true');p.add_argument('--physics-only',action='store_true');a=p.parse_args()
a.binary=a.binary.resolve();a.out=a.out.resolve();a.out.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
manifest={'started':datetime.datetime.now(datetime.timezone.utc).isoformat(),'status':'running','software_check':a.software_check,'physics_only':a.physics_only,'binary_sha256':sha(a.binary),'sources':{str(p.relative_to(HERE)):sha(p) for p in sorted((HERE/'lbm').rglob('*')) if p.is_file() and 'target' not in p.parts},'commands':[]}
manifest['scripts']={p.name:sha(p) for p in [HERE/'lbm-run.py',HERE/'lbm-analyze.py']}
def save(): (a.out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
def run(args,name):
 cmd=['taskset','-c',a.cpus,str(a.binary)]+list(map(str,args))
 if a.software_check:cmd.append('--allow-software')
 entry={'argv':cmd,'log':name+'.log'};manifest['commands'].append(entry);save();print('running',name,flush=True);start=time.monotonic()
 with (a.out/(name+'.log')).open('w') as f:r=subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT)
 entry.update(returncode=r.returncode,seconds=time.monotonic()-start);save();return r.returncode==0
try:
 manifest['environment']={x:Path(x).read_text() for x in ['/proc/loadavg','/proc/self/status'] if Path(x).exists()}
 assert run(['probe'],'probe'),'GPU probe failed'
 assert run(['compare',a.out/'compare.json'],'compare'),'GPU/reference agreement failed'
 if not a.software_check or a.physics_only:
  physics=[]
  for re in [0.1,100,300]:
   for n in [192,256]:physics.append(('steady',n,re,1))
  for n in [192,256]:
   for refine in [1,2]:physics.append(('pulse',n,100,refine))
  for n in [64,128,192,256]:
   for refine in [1,2]:physics.append(('vortex',n,100,refine))
  for kind,n,re,refine in physics:
   run(['physics',kind,n,re,refine,a.out/'physics'],f'{kind}-{n}-re{re}-r{refine}')
 if not a.software_check and not a.physics_only:
  configs=[(n,ny,c,100) for n,ny in [(192,192),(256,256),(192,30),(256,40)] for c in [0,72]]
  # Low-Re cost is separately measured on the original narrow channel.
  configs += [(192,30,0,0.1),(256,40,0,0.1)]
  for rnd,ordered in enumerate([configs,list(reversed(configs))],1):
   for n,ny,c,re in ordered:
    name=f'r{rnd}-{n}x{ny}-c{c}-re{re}'
    run(['bench',n,ny,c,a.frames,re,a.out/name],name)
 manifest['status']='complete' if all(x['returncode']==0 for x in manifest['commands']) else 'completed_with_failures'
except BaseException as e:
 manifest['status']='failed';manifest['error']=str(e);raise
finally:
 manifest['finished']=datetime.datetime.now(datetime.timezone.utc).isoformat();save()
 hashes={str(p.relative_to(a.out)):sha(p) for p in sorted(a.out.rglob('*')) if p.is_file() and p.name!='checksums.json'}
 (a.out/'checksums.json').write_text(json.dumps(hashes,indent=2)+'\n')
