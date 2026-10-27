#!/usr/bin/env python3
"""Check raw results and summarize complete frame costs, including waits."""
import csv,hashlib,json,math,sys
from pathlib import Path
root=Path(sys.argv[1]);manifest=json.loads((root/'manifest.json').read_text())
for name,wanted in json.loads((root/'checksums.json').read_text()).items():
 assert hashlib.sha256((root/name).read_bytes()).hexdigest()==wanted,name
rows=[]
for p in sorted(root.glob('r*-*.csv')):
 data=list(csv.DictReader(p.open()));meta=json.loads(p.with_suffix('.json').read_text());assert len(data)==meta['frames']
 for r in data:
  assert all(math.isfinite(float(v)) for v in r.values()),p
 def stats(key):
  vals=sorted(float(r[key]) for r in data);return {'mean':sum(vals)/len(vals),'p95':vals[math.ceil(.95*len(vals))-1]}
 row={'run':p.stem,'config':meta['config'],'cells':meta['cells'],'re_reference':meta['re_reference'],'samples':len(data)}
 for k in ['fluid_coupling_ms','whole_ms','gpu_ms','encode_ms','wait_ms','coupling_ms','rapier_ms','diagnostics_ms']:row[k]=stats(k)
 row['rho_min']=min(float(r['rho_min']) for r in data);row['rho_max']=max(float(r['rho_max']) for r in data);row['max_mach']=max(float(r['max_mach']) for r in data);row['max_area_rel']=max(float(r['max_area_rel']) for r in data);row['max_uploaded_exchange_mismatch']=max(float(r['uploaded_exchange_mismatch']) for r in data);rows.append(row)
physics=[json.loads(p.read_text()) for p in sorted((root/'physics').glob('*.json'))]
summary={'manifest_status':manifest['status'],'software_check':manifest['software_check'],'commands_failed':[x for x in manifest['commands'] if x['returncode']],'timing_runs':rows,'physics':physics}
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
lines=['# GPU LBM 原始结果汇总','','软件检查：'+str(manifest['software_check'])+'；软件结果不用于 GPU 性能结论。','','| 运行 | 流体+耦合 mean / p95 ms | 完整 mean / p95 ms | 实测最大 Mach |','| --- | ---: | ---: | ---: |']
for r in rows:
 f=r['fluid_coupling_ms'];w=r['whole_ms'];lines.append(f"| {r['run']} | {f['mean']:.3f} / {f['p95']:.3f} | {w['mean']:.3f} / {w['p95']:.3f} | {r['max_mach']:.5f} |")
lines+=['','物理误差见 summary.json；失败命令完整保留在 manifest.json。']
(root/'summary.md').write_text('\n'.join(lines)+'\n');print(json.dumps({'timing_runs':len(rows),'physics_cases':len(physics),'failed_commands':len(summary['commands_failed'])}))
