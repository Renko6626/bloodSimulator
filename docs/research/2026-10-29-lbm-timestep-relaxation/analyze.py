#!/usr/bin/env python3
"""Compare matched game frames/world positions from the diagnostic copies."""
import argparse
import csv
import json
import math
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('results', type=Path)
a = p.parse_args()

def read(path):
    with open(path) as f:
        return list(csv.DictReader(f))

def grouped(path, keys):
    frames = {}
    for r in read(path):
        key = tuple(float(r[k]) for k in keys)
        frames.setdefault(int(r['frame']), {})[key] = r
    return frames

def distribution(values):
    values = sorted(values)
    return {'mean': sum(values) / len(values), 'p95': values[math.ceil(len(values)*.95)-1], 'max': values[-1]}

summary = {'scope': 'software functional comparison; coupled difference, not analytic error or hardware performance',
           'runs': {}, 'comparisons': {}}
manifest = json.loads((a.results/'manifest.json').read_text())
for mode in ['baseline', 'candidate']:
    root = a.results / f'{mode}-results'
    status = (root/'status.txt').read_text()
    assert manifest['builds'][mode]['run_exit'] == 0, f'{mode} did not finish'
    assert status.rstrip().endswith('COMPLETE'), f'{mode} incomplete'
    for case in manifest['cases']:
        rows = read(root / f'{case}.csv')
        assert len(rows) == manifest['frames'] + manifest['warmup']
        assert all(r['finite'] == 'true' for r in rows)
        assert (int(rows[-1]['region_switches']) > 0) == (case == 'move'), f'{mode} {case}: unexpected migration coverage'
        expected = 14 if mode == 'baseline' else 10
        assert all(int(r['lattice_steps']) == expected for r in rows)
        values = lambda key: [float(r[key]) for r in rows if r[key]]
        measured = [r for r in rows if r['phase'] == 'measure']
        fields = grouped(root/f'{case}.field.csv', ['x', 'y'])
        max_speed = max(math.hypot(float(r['ux']),float(r['uy'])) for frame in fields.values() for r in frame.values())
        dt = (1.0 / 60) / expected
        summary['runs'][f'{mode}-{case}'] = {
            'rows': len(rows), 'lattice_steps_per_frame': expected,
            'density_range': [min(values('density_min')), max(values('density_max'))],
            'area_ratio_range': [min(values('area_ratio_min')),max(values('area_ratio_max'))] if values('area_ratio_min') else None,
            'max_node_wall': max(values('node_wall_max')) if values('node_wall_max') else None, 'max_robot_wall': max(values('robot_wall')),
            'max_flux_residual': max(values('residual_max')), 'max_marker_force': max(values('marker_force')),
            'contact_frames': sum(int(r['soft_contacts']) > 0 for r in rows),
            'region_switches': int(rows[-1]['region_switches']),
            'final_robot': [float(rows[-1]['robot_x']),float(rows[-1]['robot_y'])],
            'sampled_max_world_speed': max_speed,
            'sampled_max_lattice_speed': max_speed*dt/.25,
            'measure_dispatches': sorted({int(r['dispatches']) for r in measured}),
            'measure_submissions': sorted({int(r['submissions']) for r in measured}),
            'measure_readback_calls': sorted({int(r['readback_calls']) for r in measured}),
            'software_diagnostic_lbm_ms': distribution([float(r['lbm_ms']) for r in measured]),
            'mean_rho_at_samples': {frame:sum(float(r['rho']) for r in data.values())/len(data) for frame,data in fields.items()},
        }
for case in manifest['cases']:
    b = a.results/'baseline-results'
    c = a.results/'candidate-results'
    fields0 = grouped(b/f'{case}.field.csv', ['x','y'])
    fields1 = grouped(c/f'{case}.field.csv', ['x','y'])
    nodes0 = grouped(b/f'{case}.nodes.csv', ['cell','node'])
    nodes1 = grouped(c/f'{case}.nodes.csv', ['cell','node'])
    frames = []
    assert fields0.keys() == fields1.keys()
    if manifest.get('uncoupled'):
        assert not nodes0 and not nodes1
    else:
        assert fields0.keys() == nodes0.keys() == nodes1.keys()
    for frame in sorted(fields0):
        left, right = fields0[frame], fields1[frame]
        common = left.keys() & right.keys()
        assert common
        du2, ref2, drho2 = [], [], []
        for pos in common:
            x,y = left[pos],right[pos]
            assert x['time'] == y['time']
            du2.append((float(y['ux'])-float(x['ux']))**2 + (float(y['uy'])-float(x['uy']))**2)
            ref2.append(float(x['ux'])**2+float(x['uy'])**2)
            drho2.append((float(y['rho'])-float(x['rho']))**2)
        assert nodes0.get(frame, {}).keys() == nodes1.get(frame, {}).keys()
        displacements = []
        centers = {}
        for key, x in nodes0.get(frame, {}).items():
            y = nodes1[frame][key]
            dx,dy = float(y['x'])-float(x['x']),float(y['y'])-float(x['y'])
            displacements.append(math.hypot(dx,dy))
            cell = key[0]
            value = centers.setdefault(cell, [0.,0.,0])
            value[0] += dx
            value[1] += dy
            value[2] += 1
        center_d = [math.hypot(x/n,y/n) for x,y,n in centers.values()]
        frames.append({'frame':frame,'common_cells':len(common),'baseline_cells':len(left),'candidate_cells':len(right),
            'velocity_rms_difference':math.sqrt(sum(du2)/len(du2)),
            'velocity_relative_l2':math.sqrt(sum(du2)/sum(ref2)),
            'velocity_max_difference':math.sqrt(max(du2)),
            'rho_rms_difference':math.sqrt(sum(drho2)/len(drho2)),
            'rho_max_difference':math.sqrt(max(drho2)),
            'node_displacement':distribution(displacements) if displacements else None,
            'cell_center_displacement':distribution(center_d) if center_d else None})
    robot0 = summary['runs'][f'baseline-{case}']['final_robot']
    robot1 = summary['runs'][f'candidate-{case}']['final_robot']
    summary['comparisons'][case] = {'frames':frames,
        'final_robot_distance':math.dist(robot0,robot1)}
path = a.results/'summary.json'
path.write_text(json.dumps(summary,indent=2)+'\n')
print(path)
for case,result in summary['comparisons'].items():
    print(case, 'final field/node comparison:',json.dumps(result['frames'][-1]))
