#!/usr/bin/env python3
"""Compare the recorded f32 observables and matched 600-frame timing prefixes."""
import csv
import json
from pathlib import Path
import struct
import sys
from analyze import stats

root = Path(sys.argv[1])
output = Path(sys.argv[2])
keys = ['robot_x', 'robot_y', 'active_cells', 'nearby_cells8', 'soft_contacts',
        'density_min', 'density_max', 'density_sum', 'area_ratio_min',
        'area_ratio_max', 'node_wall_max', 'region_switches']
state = {}
timings = {}
for path in sorted(root.glob('godot-*.csv')):
    if '.render.' in path.name or 'smoke' in path.name:
        continue
    rows = list(csv.DictReader(path.open()))
    case = 'hold' if path.name == 'godot-hold.csv' else 'move'
    probe = list(csv.DictReader((root / 'probe' / f'{case}.csv').open()))
    mismatches = {key: sum(struct.pack('f', float(a[key])) != struct.pack('f', float(b[key]))
                           for a, b in zip(probe, rows)) for key in keys}
    state[path.name] = dict(probe_rows=len(probe), godot_rows=len(rows),
                           matched_prefix_rows=min(len(probe), len(rows)),
                           f32_mismatches=mismatches)
    if case != 'move':
        continue
    measured = [r for r in rows if r['phase'] == 'measure' and int(r['frame']) < 720]
    # Both interval ends lie in measured frames 120..719: completed callbacks 122..720.
    rendered = [r for r in csv.DictReader(Path(str(path) + '.render.csv').open())
                if 122 <= int(r['physics_frame']) <= 720]
    item = dict(physics_rows=len(measured), interval_rows=len(rendered))
    for key in ('step_wall_ms', 'prepare_ms', 'coupling_ms', 'lbm_ms', 'rapier_ms',
                'snapshot_refresh_ms', 'readback_wait_ms'):
        item[key] = stats([float(r[key]) for r in measured])
    item['frame_interval_ms'] = stats([float(r['render_interval_ms']) for r in rendered])
    item['draw_calls'] = stats([float(r['draw_calls_previous_frame']) for r in rendered])
    timings[path.name] = item
output.mkdir(parents=True, exist_ok=True)
(output / 'recorded-state-comparison.json').write_text(json.dumps(state, indent=2) + '\n')
(output / 'prefix-controls.json').write_text(json.dumps(timings, indent=2) + '\n')
