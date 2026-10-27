#!/usr/bin/env python3
"""Validate archived measurements and compare matched large-window runs."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import re
import statistics

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    with path.open() as stream:
        rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(stream)]
    assert all(math.isfinite(v) for r in rows for v in r.values()), path
    return rows


def stats(values):
    values = sorted(values)
    assert values
    return {'mean': statistics.mean(values), 'p95': values[math.ceil(len(values) * .95) - 1],
            'max': values[-1], 'min': values[0]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--results', type=Path, default=ROOT / 'results')
    root = parser.parse_args().results
    manifest = json.loads((root / 'manifest.json').read_text())
    for name, expected in manifest['sources_sha256'].items():
        assert sha(root / 'sources' / name) == expected, name
    for variant, data in manifest['variants'].items():
        for name, expected in data['modified_sha256'].items():
            assert sha(root / 'variants' / variant / name) == expected, name
    assert manifest['audits'] == {'base': 0, 'wide': 0}
    assert len(manifest['runs']) == 4
    frames, cells, samples, summaries = {}, {}, {}, {}
    for run in manifest['runs']:
        name = run['name']
        assert run['exit_code'] == 0, name
        folder = root / name
        rows = read(folder / 'drive/frames.csv')
        assert len(rows) == round(manifest['seconds'] * 60)
        assert [r['frame'] for r in rows] == list(range(1, len(rows) + 1))
        for key, threshold in [('area_error', .25), ('wall_overshoot', .15), ('exchange', 1e-7),
                               ('divergence', 1e-8), ('momentum', 1e-8)]:
            assert max(r[key] for r in rows) < threshold, (name, key)
        assert sum(r['switches'] for r in rows) >= 2
        assert len({(r['min_x'], r['min_y']) for r in rows}) == 2
        assert all(0 <= r['full_visible_cells'] <= r['visible_cells'] <= 72 for r in rows)
        stderr = (folder / 'stderr.log').read_text()
        windows = []
        for line in stderr.splitlines():
            if line.startswith('WINDOW,'):
                _, x, y, n, h, nv, np, ms = line.split(',')
                windows.append({'origin': [float(x), float(y)], 'n': int(n), 'h': float(h),
                                'velocity': int(nv), 'pressure': int(np),
                                'unknowns': int(nv) + int(np), 'init_ms': float(ms)})
        assert len(windows) == 2
        variant = manifest['variants'][run['variant']]
        assert all(w['n'] == variant['n'] and w['h'] == variant['h'] for w in windows)
        events = [r for r in rows if r['switches'] > 0]
        summaries[name] = {
            'frames': len(rows), 'switches': sum(r['switches'] for r in rows),
            'init_ms': float(re.search(r'init_ms=([\d.]+)', (folder / 'run.log').read_text())[1]),
            'max_process_rss_kib': int(re.search(r'MAX_RSS_KIB,(\d+)', stderr)[1]),
            'windows': windows, 'whole_ms': stats([r['whole_ms'] for r in rows]),
            'fluid_ms': stats([r['fluid_ms'] for r in rows]),
            'handoff_ms': stats([r['handoff_ms'] for r in events]),
            'handoff_velocity_delta': stats([r['velocity_delta'] for r in events]),
            'max_frame_diagnostics': {k: max(r[k] for r in rows) for k in
                                      ['divergence', 'momentum', 'exchange', 'area_error', 'wall_overshoot']},
            'active_cells': stats([r['active_cells'] for r in rows]),
            'visible_cell_full_weight_fraction': sum(r['full_visible_cells'] for r in rows)
                                                / sum(r['visible_cells'] for r in rows),
            'all_visible_cell_centers_full_weight_frame_fraction':
                sum(r['full_visible_cells'] == r['visible_cells'] for r in rows) / len(rows),
            'full_camera_rectangle_inside_domain_frame_fraction':
                sum(r['camera_margin'] >= 0 for r in rows) / len(rows),
            'camera_margin': stats([r['camera_margin'] for r in rows]),
            'robot_weight': stats([r['robot_weight'] for r in rows]),
        }
        frames[name] = rows
        cells[name] = read(folder / 'drive/cells.csv')
        samples[name] = read(folder / 'drive/samples.csv')
        assert len(cells[name]) == (int(manifest['seconds']) + 1) * 72
    repeat = {}
    for variant in ['base', 'wide']:
        a, b = f'{variant}-r1', f'{variant}-r2'
        repeat[variant] = {
            'max_cell_sample_absolute_difference': max(abs(x[k]-y[k])
                for x, y in zip(cells[a], cells[b], strict=True) for k in x),
            'max_flow_sample_absolute_difference': max(abs(x[k]-y[k])
                for x, y in zip(samples[a], samples[b], strict=True) for k in x),
            'max_robot_coordinate_difference': max(abs(x[k]-y[k])
                for x, y in zip(frames[a], frames[b], strict=True) for k in ['robot_x', 'robot_y']),
        }
    distances = [math.hypot(a['x']-b['x'], a['y']-b['y'])
                 for a, b in zip(cells['base-r1'], cells['wide-r1'], strict=True)]
    initial = []
    for a, b in zip(samples['base-r1'], samples['wide-r1'], strict=True):
        if a['frame'] != 0 or a['weight'] < .99999 or b['weight'] < .99999:
            continue
        speed = math.hypot(a['u'], a['v'])
        delta = math.hypot(a['u']-b['u'], a['v']-b['v'])
        initial.append({'position': [a['x'], a['y']], 'base_velocity': [a['u'], a['v']],
                        'wide_velocity': [b['u'], b['v']], 'velocity_delta': delta,
                        'delta_over_base_speed': delta / speed if speed > 1e-9 else None})
    summary = {
        'runs': summaries, 'repeatability': repeat,
        'cross_variant_cell_center_distance_world_units_at_1s_samples': stats(distances),
        'initial_common_full_weight_flow_samples': initial,
        'current_production_sources_match': all((REPO / n).exists() and sha(REPO / n) == h
                                                for n, h in manifest['sources_sha256'].items()),
    }
    (root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
