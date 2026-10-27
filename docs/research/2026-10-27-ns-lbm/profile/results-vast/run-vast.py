#!/usr/bin/env python3
"""Run only on the authorized Vast instance; source/binaries bundled beforehand."""
import datetime,hashlib,json,os,subprocess,time
from pathlib import Path
HERE=Path(__file__).resolve().parent
OUT=HERE/'results'
OUT.mkdir(exist_ok=True)
manifest={'started':datetime.datetime.now(datetime.timezone.utc).isoformat(),'commands':[],'status':'running','cpus':[2,3,4,5],'frames':120,'physical_substeps':2,'dt':1/120}
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def save(): (OUT/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
def run(cmd,name,env=None,allow_failure=False):
    entry={'argv':list(map(str,cmd)),'environment_overrides':env or {},'log':name+'.log'}
    manifest['commands'].append(entry);save();t=time.monotonic()
    print('running',name,flush=True)
    with (OUT/(name+'.log')).open('w') as f:
        r=subprocess.run(cmd,cwd=HERE,env=dict(os.environ,**(env or {})),stdout=f,stderr=subprocess.STDOUT)
    entry.update(returncode=r.returncode,wall_seconds=time.monotonic()-t);save()
    if r.returncode and not allow_failure:raise RuntimeError(name+' failed')
def snapshot():
    out={}
    for p in ['/proc/loadavg','/proc/sys/kernel/perf_event_paranoid','/proc/self/status','/sys/fs/cgroup/cpu.max','/sys/fs/cgroup/cpu.stat','/root/.vast_containerlabel']:
        try:out[p]=Path(p).read_text()
        except OSError:out[p]='unavailable'
    return out
try:
    manifest['environment_start']=snapshot()
    manifest['input_hashes']={str(p.relative_to(HERE)):sha(p) for p in sorted((HERE/'bin').glob('*')) if p.is_file()}
    manifest['sources_sha256']={str(p.relative_to(HERE)):sha(p) for p in sorted((HERE/'source').rglob('*')) if p.is_file()}
    save()
    run(['lscpu'],'cpu-info');run(['nvidia-smi','-q'],'gpu-info')
    run(['./bin/perf-probe'],'perf-permission',allow_failure=True)
    variants=[('baseline','baseline',4,False),('off4','profile',4,False),('detail4','profile',4,True),('off1','profile',1,False),('detail1','profile',1,True)]
    configs=[(n,v) for n in [192,256] for v in variants]
    for round_id,ordered in enumerate([configs,list(reversed(configs))],1):
        for n,(label,binary,threads,detail) in ordered:
            name=f'r{round_id}-{n}-{label}'
            env={'NS_THREADS':str(threads)}
            if detail:env['NS_PROFILE']='1'
            run(['taskset','-c','2-5',str(HERE/'bin'/binary),'bench',str(n),str(n),'72','120',str(OUT/name)],name,env)
    for n in [192,256]:
        name=f'sample-{n}-t4'
        run(['taskset','-c','2-5',str(HERE/'bin/profile'),'bench',str(n),str(n),'72','240',str(OUT/name)],name,
            {'NS_THREADS':'4','NS_PROFILE':'1','LD_PRELOAD':str(HERE/'bin/sample.so'),'NS_SAMPLES':str(OUT/(name+'.samples'))})
    # Existing analytic cases on the same binary and cloud CPU; timing is not used.
    run(['taskset','-c','2-5',str(HERE/'bin/profile'),'physics',str(OUT/'physics')],'physics')
    # Archive all mapped executables/libraries used for address symbolization.
    modules=set()
    for p in OUT.glob('*.samples.maps'):
        for line in p.read_text().splitlines():
            cols=line.split(maxsplit=5)
            if len(cols)==6 and 'x' in cols[1] and cols[5].startswith('/'):
                modules.add(cols[5])
    (OUT/'modules').mkdir(exist_ok=True)
    module_map={}
    import shutil
    for i,name in enumerate(sorted(modules)):
        src=Path(name);dest=OUT/'modules'/f'{i}-{src.name}'
        shutil.copy2(src,dest);module_map[name]={'file':str(dest.relative_to(OUT)),'sha256':sha(dest)}
    (OUT/'module-map.json').write_text(json.dumps(module_map,indent=2))
    manifest['status']='complete'
except BaseException as e:
    manifest['status']='failed';manifest['error']=repr(e);raise
finally:
    manifest['environment_end']=snapshot();manifest['finished']=datetime.datetime.now(datetime.timezone.utc).isoformat();save()
    hashes={str(p.relative_to(OUT)):sha(p) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='checksums.json'}
    (OUT/'checksums.json').write_text(json.dumps(hashes,indent=2))
