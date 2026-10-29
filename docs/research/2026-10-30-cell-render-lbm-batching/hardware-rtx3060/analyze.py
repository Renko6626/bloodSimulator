#!/usr/bin/env python3
import argparse,csv,json,math,collections
from pathlib import Path
CASES={'baseline-a':3600,'candidate-a':3600,'render-only':1200,'lbm-only':1200,'candidate-b':3600,'baseline-b':3600,'hud-only':1200,'mesh-only':1200}
def rows(p):
 with p.open() as f:return list(csv.DictReader(f))
def stats(xs):
 xs=sorted(xs)
 return {'n':len(xs),'mean':sum(xs)/len(xs),'p50':xs[math.ceil(.5*len(xs))-1],'p95':xs[math.ceil(.95*len(xs))-1],'p99':xs[math.ceil(.99*len(xs))-1],'min':xs[0],'max':xs[-1]}
def analyze(root):
 result={};reference=rows(root/'baseline-a.csv');monitor=rows(root/'gpu-timeline.csv')
 for name,count in CASES.items():
  assert (root/f'{name}.exit.txt').read_text().strip()=='0',name
  assert (root/f'{name}.csv.status.txt').read_text().strip()=='COMPLETE',name
  data=rows(root/f'{name}.csv');render=rows(root/f'{name}.csv.render.csv')
  assert len(data)==120+count,(name,len(data))
  expected=8 if name.startswith('candidate') or name=='lbm-only' else 20
  assert all(int(r['lattice_steps'])==14 and r['finite']=='true' for r in data)
  assert all((int(r['submissions']),int(r['dispatches']),int(r['readback_calls'])) in [(expected,33,3),(expected+2,34,4)] for r in data)
  assert all(abs(float(r['detailed_cells'])+float(r['abstract_cells'])-float(r['total_cells']))<1e-6 for r in data)
  item={'region_switches':int(data[-1]['region_switches']),'work_counts':dict(collections.Counter('/'.join(r[k] for k in ['lattice_steps','dispatches','submissions','readback_calls']) for r in data)),'spawned':sum(int(r['spawned']) for r in data),'retired':sum(int(r['retired']) for r in data),'detailed_range':[min(int(r['detailed_cells']) for r in data),max(int(r['detailed_cells']) for r in data)]}
  for label,size in [('matched600',600),('matched1200',1200),('all',count)]:
   selected=[r for r in data if 120<=int(r['frame'])<120+size]
   intervals=[r for prev,r in zip(render,render[1:]) if all(120<=int(x['physics_frame'])<120+size for x in [prev,r])]
   assert len(selected)==size and len(intervals)==size-1,(name,label,len(selected),len(intervals))
   part={'frames':size,'interval_ms':stats([float(r['render_interval_ms']) for r in intervals]),'draw_calls':stats([float(r['draw_calls_previous_frame']) for r in intervals])}
   for f in ['step_wall_ms','tick_wall_ms','snapshot_refresh_ms','physics_callback_before_csv_ms','prepare_ms','coupling_ms','lbm_ms','rapier_ms','readback_wait_ms','cell_exchange_ms']:
    part[f]=stats([float(r[f]) for r in selected])
   item[label]=part
  fields=['robot_x','robot_y','density_min','density_max','density_sum','detailed_cells','abstract_cells','region_switches','area_ratio_min','area_ratio_max','node_wall_max']
  item['trajectory_max_abs_difference_vs_baseline']={f:max(abs(float(x[f])-float(y[f])) for x,y in zip(reference,data)) for f in fields}
  start=(root/f'{name}.started.txt').read_text().strip().removesuffix('Z');end=(root/f'{name}.finished.txt').read_text().strip().removesuffix('Z')
  this_monitor=rows(root/'extra-gpu-timeline.csv') if name in ['hud-only','mesh-only'] else monitor
  ms=[{k.strip():v.strip() for k,v in r.items()} for r in this_monitor if start<=r['timestamp'].replace('/','-').replace(' ','T')<end]
  item['gpu_monitor']={'samples':len(ms),'pstates':dict(collections.Counter(r['pstate'] for r in ms))}
  for f in ['clocks.current.graphics [MHz]','clocks.current.memory [MHz]','utilization.gpu [%]']:
   if ms:item['gpu_monitor'][f]=stats([float(r[f].split()[0]) for r in ms])
  result[name]=item
  print(name,'all',item['all']['interval_ms'],'LBM',item['all']['lbm_ms']['mean'],'wait',item['all']['readback_wait_ms']['mean'],'draw',item['all']['draw_calls']['mean'])
 pooled={}
 for label,names in [('baseline',['baseline-a','baseline-b']),('candidate',['candidate-a','candidate-b'])]:
  intervals=[]
  for name in names:
   render=rows(root/f'{name}.csv.render.csv')
   intervals.extend(float(r['render_interval_ms']) for prev,r in zip(render,render[1:]) if 120<=int(prev['physics_frame'])<3720 and 120<=int(r['physics_frame'])<3720)
  pooled[label]={'interval_ms':stats(intervals),'fraction_within_60hz_budget':sum(v<=1000/60 for v in intervals)/len(intervals)}
 result['pooled_main']=pooled
 (root/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('root',type=Path);analyze(p.parse_args().root)
