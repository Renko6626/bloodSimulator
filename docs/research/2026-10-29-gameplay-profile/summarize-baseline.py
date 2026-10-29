#!/usr/bin/env python3
"""Same 600 measured physics frames and 599 intervals; no sum of percentiles."""
import csv
import json
from pathlib import Path
import statistics
import math
import sys
root = Path(sys.argv[1])
out = Path(sys.argv[2])

def stats(values):
    v = sorted(values)
    return {'mean': statistics.mean(v), 'p95': v[math.ceil(.95*len(v))-1], 'n':len(v)}

result = {}
for name in ['probe/move.csv','godot-move.csv','godot-headless-move.csv','godot-no-draw-move.csv','godot-vblank-off-move.csv']:
    rows = [r for r in csv.DictReader((root/name).open()) if r['phase']=='measure'][:600]
    assert [int(r['frame']) for r in rows] == list(range(120,720))
    fields = ['step_wall_ms','prepare_ms','coupling_ms','lbm_ms','rapier_ms','readback_wait_ms','snapshot_ms','tick_wall_ms','snapshot_refresh_ms']
    item = {k:stats([float(r[k]) for r in rows]) for k in fields if k in rows[0]}
    item['step_unattributed_ms'] = stats([float(r['step_wall_ms'])-sum(float(r[k]) for k in ['prepare_ms','coupling_ms','lbm_ms','rapier_ms']) for r in rows])
    item['lbm_excluding_wait_ms'] = stats([float(r['lbm_ms'])-float(r['readback_wait_ms']) for r in rows])
    if 'tick_wall_ms' in rows[0]:
        item['tick_excluding_step_ms'] = stats([float(r['tick_wall_ms'])-float(r['step_wall_ms']) for r in rows])
        render = list(csv.DictReader(Path(str(root/name)+'.render.csv').open()))
        intervals = [r for r in render if 122 <= int(r['physics_frame']) <= 720]
        assert len(intervals)==599
        item['frame_interval_ms']=stats([float(r['render_interval_ms']) for r in intervals])
        # The interval ending at completed physics N contains physics row N-1.
        # This residual includes previous draw+engine+logging+other work, and is
        # not a pure GPU/render duration.
        by_frame = {int(r['frame']):r for r in rows}
        item['interval_excluding_current_tick_refresh_ms']=stats([
            float(r['render_interval_ms'])-float(by_frame[int(r['physics_frame'])-1]['tick_wall_ms'])-float(by_frame[int(r['physics_frame'])-1]['snapshot_refresh_ms']) for r in intervals])
    result[name]=item
out.write_text(json.dumps(result,indent=2)+'\n')
for name,item in result.items():
    print(name, ' '.join(f'{k}={v["mean"]:.3f}' for k,v in item.items()))
