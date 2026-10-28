#!/usr/bin/env python3
import csv,hashlib,json,math,statistics,sys
from pathlib import Path
root=Path(__file__).resolve().parent
path=Path(sys.argv[1]) if len(sys.argv)>1 else root/'hardware-rtx3060-20261029/results/bench/frames.csv'
rows=list(csv.DictReader(path.open()))
assert len(rows)==1000

def stats(values):
    a=sorted(values);return dict(avg=math.fsum(a)/len(a),median=statistics.median(a),p95=a[math.ceil(.95*len(a))-1],max=a[-1])
keys=['total_wall_ms','budget_60hz_percent','profile_ms','enqueue_first7_cpu_ms','field1_wall_ms','enqueue_second7_cpu_ms','raw_read_wall_ms','cpu_mean_ms','normalization_enqueue_cpu_ms','field2_wall_ms','field_validity_cpu_ms','upload_cpu_ms','encoding_cpu_ms','submission_cpu_ms','readback_wait_wall_ms','readback_decode_cpu_ms']
counts=['lattice_steps','dispatches','submissions','upload_calls','upload_bytes','readback_calls','readback_bytes']
summary=dict(input_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),measured_frames_per_mode=500,warmup_frames_per_mode=100,percentile_definition='nearest-rank ceil(.95*n)-1',modes={})
for mode in ['uncorrected','mean_controlled']:
    data=[r for r in rows if r['mode']==mode]
    assert len(data)==500 and [int(r['sample']) for r in data]==list(range(500)) and [int(r['iteration']) for r in data]==list(range(100,600))
    summary['modes'][mode]=dict(timings={k:stats([float(r[k]) for r in data]) for k in keys},counts={k:dict(min=min(int(r[k]) for r in data),max=max(int(r[k]) for r in data)) for k in counts})
a=summary['modes']['uncorrected']['timings']['total_wall_ms'];b=summary['modes']['mean_controlled']['timings']['total_wall_ms']
summary['combined_overhead_ms']={k:b[k]-a[k] for k in a}
(root/'results/benchmark-analysis.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
