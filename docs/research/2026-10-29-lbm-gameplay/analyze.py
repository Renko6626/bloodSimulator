#!/usr/bin/env python3
"""Summarize actual frames, retaining failure prefixes and load/shift strata."""
import csv
import json
import math
from pathlib import Path
import statistics
import sys


def stats(values):
    values = sorted(v for v in values if math.isfinite(v))
    if not values:
        return None
    return dict(n=len(values), mean=sum(values) / len(values),
                median=statistics.median(values),
                p95=values[math.ceil(len(values) * .95) - 1],
                p99=values[math.ceil(len(values) * .99) - 1], max=values[-1], min=values[0])


def summarize(path):
    rows = list(csv.DictReader(path.open()))
    if not rows:
        return dict(rows=0)
    render = path.name.endswith('.render.csv')
    if render:
        # _process records the number of completed physics callbacks; frame N-1
        # is the latest corresponding physics CSV row. An interval spans the
        # previous draw and current physics, so both ends must be measured.
        main = path.with_name(path.name.removesuffix('.render.csv'))
        physics = {int(r['frame']): r for r in csv.DictReader(main.open())}
        previous_measured = False
        for row in rows:
            current = physics.get(int(row['physics_frame']) - 1, {})
            current_measured = current.get('phase') == 'measure'
            row['phase'] = ('measure' if current_measured and previous_measured
                            and float(row['render_interval_ms']) > 0 else 'warmup')
            previous_measured = current_measured
            for key in ('active_cells', 'nearby_cells8', 'soft_contacts', 'region_switches'):
                row[key] = current.get(key, '0')
    numeric = {}
    for key in rows[0]:
        try:
            numeric[key] = [float(r[key]) for r in rows]
        except (ValueError, TypeError):
            pass
    measured = [i for i, r in enumerate(rows) if r.get('phase', 'measure') == 'measure']
    previous = 0
    shift_frames = set()
    for i, value in enumerate(numeric.get('region_switches', [])):
        if value > previous:
            shift_frames.add(i)
        previous = value
    groups = {'all_measured': measured}
    if 'active_cells' in numeric:
        active = numeric['active_cells']
        groups.update({
            'active_0': [i for i in measured if active[i] == 0],
            'active_1_23': [i for i in measured if 0 < active[i] < 24],
            'active_24_plus': [i for i in measured if active[i] >= 24],
        })
    if 'soft_contacts' in numeric:
        contacts = numeric['soft_contacts']
        groups['contact_frames'] = [i for i in measured if contacts[i] > 0]
        if 'active_cells' in numeric:
            groups['active_24_plus_and_contacts'] = [i for i in measured
                                                    if active[i] >= 24 and contacts[i] > 0]
    if 'region_switches' in numeric:
        groups['migration_frames'] = [i for i in measured if i in shift_frames]
        groups['ordinary_frames'] = [i for i in measured if i not in shift_frames]
    if measured:
        for start in range(0, len(measured), 600):
            groups[f'measured_frames_{start}_{start+599}'] = measured[start:start + 600]
    timing_keys = [k for k in numeric if k.endswith('_ms')]
    load_keys = [k for k in ('active_cells', 'nearby_cells8', 'soft_contacts',
                            'density_min', 'density_max', 'area_ratio_min', 'area_ratio_max',
                            'node_wall_max', 'cell_speed_max', 'marker_force', 'residual_max',
                            'readback_calls') if k in numeric]
    def group(indices):
        result = {'frames': len(indices)}
        for key in timing_keys + load_keys:
            result[key] = stats([numeric[key][i] for i in indices])
        if 'step_wall_ms' in numeric:
            result['over_16_667ms'] = sum(numeric['step_wall_ms'][i] > 1000/60 for i in indices)
        return result
    result = dict(rows=len(rows), measured_frames=len(measured),
                last_row=rows[-1],
                actual_robot_x_range=stats(numeric.get('robot_x', [])),
                actual_robot_y_range=stats(numeric.get('robot_y', [])),
                migration_frames_including_warmup=sorted(shift_frames),
                strata={name: group(indices) for name, indices in groups.items()})
    status = Path(str(path).removesuffix('.render.csv') + '.status.txt') if render else Path(str(path) + '.status.txt')
    if status.exists():
        result['status'] = status.read_text()
        result['complete_marker'] = result['status'].strip() == 'COMPLETE'
    return result


if __name__ == '__main__':
    root = Path(sys.argv[1])
    result = {p.name: summarize(p) for p in sorted(root.glob('*.csv'))}
    status = root / 'status.txt'
    if status.exists():
        result['status'] = status.read_text()
        result['complete_marker'] = 'COMPLETE' in result['status']
    text = json.dumps(result, ensure_ascii=False, indent=2, allow_nan=False) + '\n'
    if len(sys.argv) > 2:
        Path(sys.argv[2]).write_text(text)
    else:
        print(text, end='')
