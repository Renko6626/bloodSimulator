#!/usr/bin/env python3
"""Validate archived runs and separate solve throughput from trajectory drift."""
import argparse
from collections import Counter
import csv
import hashlib
import json
import math
from pathlib import Path
import re
import statistics

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]


def read(path):
    with path.open() as stream:
        rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(stream)]
    assert all(math.isfinite(v) for r in rows for v in r.values()), path
    return rows


def stats(values):
    values = sorted(values)
    assert values
    return {'mean': statistics.mean(values), 'p95': values[math.ceil(len(values) * .95) - 1], 'max': values[-1]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--results', type=Path, default=ROOT / 'results')
    args = parser.parse_args()
    root = args.results
    manifest = json.loads((root / 'manifest.json').read_text())
    assert len(manifest['runs']) == 6 and all(r['exit_code'] == 0 for r in manifest['runs'])
    for group in ['sources_sha256', 'production_sha256']:
        for p, h in manifest[group].items():
            assert hashlib.sha256((REPO / p).read_bytes()).hexdigest() == h, p
    summary = {'cases': {}, 'comparisons': {}, 'audits': {}}
    for run in manifest['runs']:
        name = run['name']
        folder = root / name
        frames = read(folder / 'frames.csv')
        stages = read(folder / 'stages.csv')
        deep = read(folder / 'deep.csv')
        assert len(frames) == len(stages) == len(deep) == round(manifest['seconds'] * 60)
        assert [r['frame'] for r in frames] == list(range(1, len(frames) + 1))
        assert max(r['max_area_error'] for r in frames) < .25
        assert max(r['max_wall_overshoot'] for r in frames) < .15
        assert max(r['exchange'] for r in frames) < 1e-7
        assert all(r['enabled'] == 1 for r in stages)
        assert all(r['analytic_samples'] == (72 * 12 * 2 - r['zero_drag_nodes']) / 2 for r in stages)
        cells = read(folder / 'cells.csv')
        final = [r for r in cells if r['frame'] == len(frames)]
        assert len(final) == 72
        counts = Counter((int(r['from']), int(r['to'])) for r in read(folder / 'crossings.csv'))
        log = (folder / 'stderr.log').read_text()
        factors = re.findall(r'FACTOR,(\d+),(\d+),(\d+),(\d+),(\d+)', log)
        assert len(factors) == 2
        case = {
            'whole_ms': stats([r['whole_ms'] for r in frames]),
            'fluid_ms': stats([r['fluid_ms'] for r in frames]),
            'iterations': stats([r['iterations'] for r in frames]),
            'stages': {k: stats([r[k] for r in stages]) for k in
                       ['network_ms', 'prepare_ms', 'robot_ms', 'advection_ms', 'fluid_ms', 'rapier_ms']},
            'active_cells': [min(r['active'] for r in frames), max(r['active'] for r in frames)],
            'max_diagnostics': {k: max(r[k] for r in frames) for k in
                                ['divergence_peak', 'momentum_peak', 'exchange', 'max_area_error',
                                 'max_wall_overshoot', 'grid_speed_peak', 'force_peak', 'flux_relative_peak']},
            'incoming_volume': sum(r['in_volume'] for r in frames),
            'absolute_net_volume': sum(r['abs_net_volume'] for r in frames),
            'upper_outlet_fraction': sum(r['upper_volume'] for r in frames) /
                                     sum(r['upper_volume'] + r['lower_volume'] for r in frames),
            'fork_crossings': [counts.get((0, 1), 0), counts.get((0, 4), 0)],
            'mean_cell_distance': statistics.mean(r['distance'] for r in final),
            'initialization_ms': int(re.search(r'INIT_NS,(\d+)', log)[1]) / 1e6,
            'max_process_rss_kib': int(re.search(r'MAX_RSS_KIB,(\d+)', log)[1]),
            'factors': [{'axis': int(a), 'size': int(n), 'entries': int(e),
                         'reported_payload_bytes': int(b), 'setup_ms': int(t) / 1e6}
                        for a, n, e, b, t in sorted(factors)],
            'factor_byte_scope': 'numeric only; symbolic metadata excluded' if run['variant'] == 'faer'
                                 else 'factor numeric+index array payload; Vec headers/capacity excluded',
        }
        if run['mode'] == 'fluid':
            names = ['grid_solve', 'velocity_inverse', 'factor_total_work', 'u_solve',
                     'v_solve', 'max_component_per_inverse', 'full_diagnostics']
            case['deep_ms'] = {k: stats([r[k + '_ns'] / 1e6 for r in deep]) for k in names}
            case['inverse_calls_per_frame'] = statistics.mean(r['velocity_inverse_calls'] for r in deep)
            case['inverse_us_per_call'] = sum(r['velocity_inverse_ns'] for r in deep) / sum(r['velocity_inverse_calls'] for r in deep) / 1e3
            case['component_us_per_solve'] = sum(r['factor_total_work_ns'] for r in deep) / sum(r['factor_total_work_calls'] for r in deep) / 1e3
            assert all(r['velocity_inverse_calls'] == f['iterations'] + 6 for r, f in zip(deep, frames))
        reference = root / (run['variant'] + '-r1')
        original = read(reference / 'frames.csv')
        semantic = [k for k in frames[0] if k not in ['whole_ms', 'fluid_ms']]
        assert all(all(a[k] == b[k] for k in semantic) for a, b in zip(frames, original)), name
        for filename in ['fingerprints.csv', 'cells.csv', 'arrows.csv', 'crossings.csv']:
            assert (folder / filename).read_bytes() == (reference / filename).read_bytes(), (name, filename)
        case['same_backend_replay_matches'] = True
        summary['cases'][name] = case
    for repeat in [1, 2]:
        a = summary['cases'][f'custom-r{repeat}']
        b = summary['cases'][f'faer-r{repeat}']
        ac = read(root / f'custom-r{repeat}' / 'cells.csv')
        bc = read(root / f'faer-r{repeat}' / 'cells.csv')
        assert len(ac) == len(bc) and all((x['frame'], x['cell']) == (y['frame'], y['cell']) for x, y in zip(ac, bc))
        final_frame = round(manifest['seconds'] * 60)
        final = [(x, y) for x, y in zip(ac, bc) if x['frame'] == final_frame]
        aa = read(root / f'custom-r{repeat}' / 'arrows.csv')
        ba = read(root / f'faer-r{repeat}' / 'arrows.csv')
        assert len(aa) == len(ba) and all((x['frame'], x['arrow']) == (y['frame'], y['arrow']) for x, y in zip(aa, ba))
        summary['comparisons'][f'round{repeat}'] = {
            'whole_saved_ms': a['whole_ms']['mean'] - b['whole_ms']['mean'],
            'whole_speedup': a['whole_ms']['mean'] / b['whole_ms']['mean'],
            'fluid_saved_ms': a['fluid_ms']['mean'] - b['fluid_ms']['mean'],
            'fluid_speedup': a['fluid_ms']['mean'] / b['fluid_ms']['mean'],
            'incoming_volume_relative_change': b['incoming_volume'] / a['incoming_volume'] - 1,
            'mean_cell_distance_relative_change': b['mean_cell_distance'] / a['mean_cell_distance'] - 1,
            'upper_outlet_fraction_change': b['upper_outlet_fraction'] - a['upper_outlet_fraction'],
            'max_sampled_cell_position_difference': max(math.hypot(x['x'] - y['x'], x['y'] - y['y']) for x, y in zip(ac, bc)),
            'different_edges_at_end': sum(x['edge'] != y['edge'] for x, y in final),
            'cell_samples_files_equal': (root / f'custom-r{repeat}/cells.csv').read_bytes() ==
                                       (root / f'faer-r{repeat}/cells.csv').read_bytes(),
            'crossing_events_files_equal': (root / f'custom-r{repeat}/crossings.csv').read_bytes() ==
                                           (root / f'faer-r{repeat}/crossings.csv').read_bytes(),
            'max_sampled_arrow_velocity_difference': max(math.hypot(x['vx'] - y['vx'], x['vy'] - y['vy']) for x, y in zip(aa, ba)),
            'snapshot_files_equal': (root / f'custom-r{repeat}/fingerprints.csv').read_bytes() ==
                                    (root / f'faer-r{repeat}/fingerprints.csv').read_bytes(),
        }
    for variant in ['custom', 'faer']:
        log = (root / f'{variant}-audit.log').read_text()
        assert '5 passed; 0 failed' in log, variant
        match = re.search(r'FAER_AUDIT custom_max_abs=(\S+) matrix_residual_max_abs=(\S+)', log)
        assert match
        summary['audits'][variant] = {'max_velocity_abs_difference': float(match[1]),
                                       'max_matrix_abs_residual': float(match[2])}
    (root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
