#!/usr/bin/env python3
"""Summarize real process intervals; stage timers overlap and are not GPU-only."""
import argparse,csv,json,math
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('root',type=Path);a=p.parse_args()
def rows(path):
 with open(path) as f:return list(csv.DictReader(f))
def stats(values):
 vs=sorted(values)
 if not vs:return None
 return {'n':len(vs),'mean':sum(vs)/len(vs),'p50':vs[math.ceil(.5*len(vs))-1],
         'p95':vs[math.ceil(.95*len(vs))-1],'p99':vs[math.ceil(.99*len(vs))-1],'max':vs[-1]}
cases={'baseline-lbm':600,'current-lbm-full':3600,'current-lbm-local':3600,'current-world-full':600,'current-world-local':600}
summary={}
for name,count in cases.items():
 root=a.root
 result={'exit':(root/(name+'.exit.txt')).read_text().strip(),
         'status':(root/(name+'.csv.status.txt')).read_text().strip() if (root/(name+'.csv.status.txt')).exists() else 'MISSING'}
 summary[name]=result
 if result['exit']!='0' or result['status']!='COMPLETE':continue
 data=rows(root/(name+'.csv'));render=rows(root/(name+'.csv.render.csv'))
 assert len(data)==120+count,(name,len(data))
 if 'lbm' in name:
  assert all(int(r['lattice_steps'])==14 and r['finite']=='true' for r in data)
 if name.endswith('local'):
  for r in data:
   assert abs(float(r['detailed_cells'])+float(r['abstract_cells'])-float(r['total_cells']))<1e-6
 for label,size in [('matched600',600),('all',count)]:
  selected=[r for r in data if r['phase']=='measure' and 120<=int(r['frame'])<120+size]
  intervals=[];calls=[]
  for previous,current in zip(render,render[1:]):
   if all(120<=int(r['physics_frame'])<120+size for r in [previous,current]):
    intervals.append(float(current['render_interval_ms']));calls.append(float(current['draw_calls_previous_frame']))
  item={'frames':len(selected),'process_interval_ms':stats(intervals),'draw_calls':stats(calls)}
  for field in ['step_wall_ms','tick_wall_ms','snapshot_refresh_ms','physics_callback_before_csv_ms']:
   item[field]=stats([float(r[field]) for r in selected])
  if 'lbm' in name:
   for field in ['prepare_ms','coupling_ms','lbm_ms','rapier_ms','readback_wait_ms']:
    item[field]=stats([float(r[field]) for r in selected])
  result[label]=item
 if 'lbm' in name:
  result['region_switches']=int(data[-1]['region_switches'])
  assert result['region_switches']>0
  result['density_range']=[min(float(r['density_min']) for r in data),max(float(r['density_max']) for r in data)]
  result['area_ratio_range']=[min(float(r['area_ratio_min']) for r in data if r['area_ratio_min']),max(float(r['area_ratio_max']) for r in data if r['area_ratio_max'])]
  result['max_node_wall']=max(float(r['node_wall_max']) for r in data if r['node_wall_max'])
  result['active_marker_cells_range']=[min(int(r['active_cells']) for r in data),max(int(r['active_cells']) for r in data)]
 if name.endswith('local'):
  result['detailed_range']=[min(int(r['detailed_cells']) for r in data),max(int(r['detailed_cells']) for r in data)]
  result['abstract_range']=[min(float(r['abstract_cells']) for r in data),max(float(r['abstract_cells']) for r in data)]
  result['spawned']=sum(int(r['spawned']) for r in data);result['retired']=sum(int(r['retired']) for r in data)
  result['exchange_ms']=stats([float(r['cell_exchange_ms']) for r in data])
  handoffs=[r for r in data if int(r['spawned'])+int(r['retired'])]
  result['handoff_step_ms']=stats([float(r['step_wall_ms']) for r in handoffs])
 else:result['detailed_range']=[72,72]
 result['final_robot']=[float(data[-1]['robot_x']),float(data[-1]['robot_y'])]
(a.root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
for name,r in summary.items():
 print(name,r['exit'],r['status'],r.get('all',{}).get('process_interval_ms'),r.get('detailed_range'))
