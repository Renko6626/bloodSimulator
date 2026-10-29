#!/usr/bin/env python3
"""Summarize bounded profiling runs, including instrumentation and state checks."""
import csv,json,math,re,statistics,struct,sys
from pathlib import Path
root=Path(sys.argv[1]); destination=Path(sys.argv[2])

def stats(values):
    v=sorted(x for x in values if math.isfinite(x))
    if not v: return None
    return dict(n=len(v),mean=statistics.mean(v),p95=v[math.ceil(.95*len(v))-1],max=v[-1],min=v[0])

def fields_summary(rows):
    result={}
    if not rows: return result
    for k in rows[0]:
        if not (k.endswith('_ms') or k in ['passes','draw_calls_previous_frame','active_cells','soft_contacts','submissions','readback_calls']): continue
        try: result[k]=stats([float(r[k]) for r in rows])
        except (KeyError,ValueError): pass
    return result

def selected_frames(path, gd=False):
    main=path.with_name(path.name.removesuffix('.gd.csv')+'.csv') if gd else path.with_name(path.name.removesuffix('.render.csv'))
    rows=list(csv.DictReader(main.open()))
    measured=[int(r['frame']) for r in rows if r['phase']=='measure']
    return min(measured)+2,max(measured)+1

result={'physics':{},'render':{},'gdscript':{},'rust_detail':{},'state_comparison':{},'exits':{}}
for p in sorted(root.glob('*.csv')):
    if p.name=='waits.csv': continue
    rows=list(csv.DictReader(p.open()))
    if not rows: continue
    if p.name.endswith('.render.csv'):
        lo,hi=selected_frames(p)
        selected=[r for r in rows if lo<=int(r['physics_frame'])<=hi]
        result['render'][p.name]=fields_summary(selected)
    elif p.name.endswith('.gd.csv'):
        lo,hi=selected_frames(p,True)
        selected=[r for r in rows if lo<=int(r['physics_frame'])<=hi]
        result['gdscript'][p.name]=fields_summary(selected)
    elif 'phase' in rows[0]:
        selected=[r for r in rows if r['phase']=='measure']
        result['physics'][p.name]=fields_summary(selected)
for name in ['baseline-probe','detail-probe','probe-long']:
    p=root/name/'move.csv'
    if p.exists(): result['physics'][name]=fields_summary([r for r in csv.DictReader(p.open()) if r['phase']=='measure'])
for p in sorted(root.glob('*.log')):
    recorded=[];gpu=[];inner=None;advection=[]
    for line in p.read_text(errors='replace').splitlines():
        if not line.startswith('PROFILE_'): continue
        tag=line.split()[0]
        values={k:float(v) for k,v in re.findall(r'(\w+)=(-?\d+(?:\.\d+)?)',line)}
        if tag=='PROFILE_GPU': gpu.append(values)
        elif tag=='PROFILE_INNER': inner=values['inner_ms']
        elif tag=='PROFILE_ADVECTION': advection.append(values['ms'])
        elif tag=='PROFILE_STEP':
            if inner is not None:
                values['inner_ms']=inner
                values['outside_inner_ms']=values['total_ms']-inner
            values['advection_ms']=sum(advection)
            if gpu:
                values['kernels_ms']=sum(v['kernels_ms'] for v in gpu)
                values['gpu_spans_ms']=sum(v['span_ms'] for v in gpu)
                values['passes']=sum(v['passes'] for v in gpu)
            if values['frame']>=120: recorded.append(values)
            gpu=[];inner=None;advection=[]
    if recorded: result['rust_detail'][p.name]=fields_summary(recorded)
reference_path=root/'baseline-probe'/'move.csv'
if reference_path.exists():
    reference=list(csv.DictReader(reference_path.open()))
    keys=['robot_x','robot_y','active_cells','nearby_cells8','soft_contacts','density_min','density_max','density_sum','area_ratio_min','area_ratio_max','node_wall_max','region_switches']
    for p in sorted(root.glob('*.csv')):
        if p.name.endswith(('.gd.csv','.render.csv')) or p.name=='waits.csv': continue
        rows=list(csv.DictReader(p.open()))
        if not rows or 'phase' not in rows[0]: continue
        mismatch={k:sum(struct.pack('f',float(a[k]))!=struct.pack('f',float(b[k])) for a,b in zip(reference,rows)) for k in keys}
        result['state_comparison'][p.name]={'reference_rows':len(reference),'rows':len(rows),'matched_prefix_rows':min(len(reference),len(rows)),'f32_mismatches':mismatch}
for p in sorted(root.glob('*.exit.txt')): result['exits'][p.name]=p.read_text().strip()
p=root/'waits.csv'
if p.exists():
    groups={}
    for r in csv.DictReader(p.open()): groups.setdefault((r['tid'],r['name']),[]).append(r)
    result['waits']=[{'tid':tid,'name':name,'wall_ms':stats([float(r['wall_ms']) for r in rows]),'thread_cpu_ms':stats([float(r['thread_cpu_ms']) for r in rows]),'wall_sum_ms':sum(float(r['wall_ms']) for r in rows)} for (tid,name),rows in groups.items()]
destination.write_text(json.dumps(result,indent=2)+'\n')
for kind in ['physics','render','gdscript','rust_detail']:
    for name,values in result[kind].items():
        print(kind,name,' '.join(f'{k}={v["mean"]:.3f}' for k,v in values.items() if v))
print('exits',result['exits'])
