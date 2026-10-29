#!/usr/bin/env python3
"""Read-only telemetry summaries and the full recorded long-run comparison."""
import csv,json,statistics,struct,sys
from pathlib import Path
from collections import Counter
root=Path(sys.argv[1]);out=Path(sys.argv[2])
telemetry={}
for name in ['no-aa','normal-repeat','no-world-long','probe-long']:
    rows=list(csv.DictReader((root/(name+'.gpu.csv')).open(),skipinitialspace=True))
    clocks=[float(r['clocks.current.graphics [MHz]'].split()[0]) for r in rows]
    telemetry[name]={'samples':len(rows),'pstates':dict(Counter(r['pstate'] for r in rows)),
                     'clock_min_mhz':min(clocks),'clock_max_mhz':max(clocks),'clock_median_mhz':statistics.median(clocks)}
(out/'clock-summary.json').write_text(json.dumps(telemetry,indent=2)+'\n')
reference=list(csv.DictReader((root/'probe-long/move.csv').open()))
actual=list(csv.DictReader((root/'no-world-long.csv').open()))
keys=['robot_x','robot_y','active_cells','nearby_cells8','soft_contacts','density_min','density_max','density_sum','area_ratio_min','area_ratio_max','node_wall_max','region_switches']
comparison={'reference_rows':len(reference),'godot_rows':len(actual),'f32_mismatches':{
    k:sum(struct.pack('f',float(a[k]))!=struct.pack('f',float(b[k])) for a,b in zip(reference,actual)) for k in keys}}
(out/'long-state-comparison.json').write_text(json.dumps(comparison,indent=2)+'\n')
print(json.dumps(telemetry,indent=2));print(json.dumps(comparison,indent=2))
