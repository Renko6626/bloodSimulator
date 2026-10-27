#!/usr/bin/env python3
"""Pool the two measured rounds; p95 comes from raw frames, never phase sums."""
import collections,csv,json,math
from pathlib import Path
HERE=Path(__file__).resolve().parent
root=HERE/'retrieved/results'
groups=collections.defaultdict(list)
for p in sorted(root.glob('r*-*.csv')):
    meta=json.loads(p.with_suffix('.json').read_text())
    rows=list(csv.DictReader(p.open()))
    assert len(rows)==meta['frames']==120
    for row in rows:
        assert all(math.isfinite(float(v)) for v in row.values())
    key=(meta['config']['nx'],meta['config']['ny'],meta['cells'],meta['re_reference'])
    groups[key].extend(rows)
pooled=[]
for key,rows in sorted(groups.items()):
    assert len(rows)==240
    def stats(k):
        vals=sorted(float(r[k]) for r in rows)
        return {'mean':sum(vals)/len(vals),'p95':vals[math.ceil(.95*len(vals))-1]}
    item=dict(zip(['nx','ny','cells','re'],key));item['frames']=len(rows)
    for k in ['fluid_coupling_ms','whole_ms','gpu_ms','upload_ms','encode_ms','submit_ms','wait_ms','decode_ms','advance_ms','coupling_ms','rapier_ms','diagnostics_ms']:
        item[k]=stats(k)
    for k in ['max_mach','max_area_rel','uploaded_exchange_mismatch']:
        item['max_uploaded_exchange_mismatch' if k=='uploaded_exchange_mismatch' else k]=max(float(r[k]) for r in rows)
    item['rho_min']=min(float(r['rho_min']) for r in rows)
    item['rho_max']=max(float(r['rho_max']) for r in rows)
    pooled.append(item)
(HERE/'pooled-summary.json').write_text(json.dumps(pooled,indent=2)+'\n')
print(json.dumps({'configs':len(pooled),'frames':sum(x['frames'] for x in pooled)}))
