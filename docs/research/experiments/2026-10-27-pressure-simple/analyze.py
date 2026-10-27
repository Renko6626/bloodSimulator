#!/usr/bin/env python3
"""Check the SIMPLE spike and retain negative performance results."""
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
    root = parser.parse_args().results
    manifest = json.loads((root / 'manifest.json').read_text())
    assert len(manifest['runs']) == 6 and all(r['exit_code'] == 0 for r in manifest['runs'])
    assert manifest['audits'] == {'cg': 0, 'simple': 0}
    for key in ['sources_sha256', 'production_sha256']:
        for path, expected in manifest[key].items():
            assert hashlib.sha256((REPO / path).read_bytes()).hexdigest() == expected, path
    summary = {'cases': {}, 'comparisons': {}}
    for run in manifest['runs']:
        name = run['name']
        folder = root / name
        frames = read(folder / 'frames.csv')
        deep = read(folder / 'deep.csv')
        stages = read(folder / 'stages.csv')
        assert len(frames) == len(deep) == len(stages) == round(manifest['seconds'] * 60)
        assert [r['frame'] for r in frames] == list(range(1, len(frames) + 1))
        assert max(r['max_area_error'] for r in frames) < .25
        assert max(r['max_wall_overshoot'] for r in frames) < .15
        assert max(r['exchange'] for r in frames) < 1e-7
        assert all(r['enabled'] == 1 and r['analytic_samples'] == (72 * 12 * 2 - r['zero_drag_nodes']) / 2 for r in stages)
        cells = read(folder / 'cells.csv')
        final = [r for r in cells if r['frame'] == len(frames)]
        assert len(final) == 72
        log = (folder / 'stderr.log').read_text()
        case = {'whole_ms': stats([r['whole_ms'] for r in frames]),
                'fluid_ms': stats([r['fluid_ms'] for r in frames]),
                'iterations': stats([r['iterations'] for r in frames]),
                'mean_cell_distance': statistics.mean(r['distance'] for r in final),
                'incoming_volume': sum(r['in_volume'] for r in frames),
                'upper_fraction': sum(r['upper_volume'] for r in frames) / sum(r['upper_volume'] + r['lower_volume'] for r in frames),
                'max_diagnostics': {k: max(r[k] for r in frames) for k in
                                    ['divergence_peak', 'momentum_peak', 'exchange', 'flux_relative_peak',
                                     'grid_speed_peak', 'force_peak', 'max_area_error', 'max_wall_overshoot']},
                'initialization_ms': int(re.search(r'INIT_NS,(\d+)', log)[1]) / 1e6}
        if run['variant'] == 'simple':
            match = re.search(r'PRESSURE_FACTOR,(\d+),(\d+),(\d+),(\d+)', log)
            assert match
            case['pressure_factor'] = dict(zip(['size', 'entries', 'payload_bytes', 'setup_ns'], map(int, match.groups())))
        if run['mode'] == 'fluid':
            case['deep_ms'] = {k: stats([r[k + '_ns'] / 1e6 for r in deep]) for k in
                               ['grid_solve', 'velocity_inverse', 'pressure_preconditioner']}
            case['inverse_calls_per_frame'] = statistics.mean(r['velocity_inverse_calls'] for r in deep)
            case['preconditioner_calls_per_frame'] = statistics.mean(r['pressure_preconditioner_calls'] for r in deep)
            assert all(r['velocity_inverse_calls'] == f['iterations'] + 6 for r, f in zip(deep, frames))
        reference = root / (run['variant'] + '-r1')
        baseline = read(reference / 'frames.csv')
        semantic = [k for k in frames[0] if k not in ['whole_ms', 'fluid_ms']]
        assert all(all(a[k] == b[k] for k in semantic) for a, b in zip(frames, baseline)), name
        for filename in ['fingerprints.csv', 'cells.csv', 'arrows.csv', 'crossings.csv']:
            assert (folder / filename).read_bytes() == (reference / filename).read_bytes(), (name, filename)
        case['same_backend_replay_matches'] = True
        summary['cases'][name] = case
    for repeat in [1, 2]:
        a = summary['cases'][f'cg-r{repeat}']
        b = summary['cases'][f'simple-r{repeat}']
        ac = read(root / f'cg-r{repeat}/cells.csv')
        bc = read(root / f'simple-r{repeat}/cells.csv')
        assert len(ac) == len(bc) and all((x['frame'], x['cell']) == (y['frame'], y['cell']) for x, y in zip(ac, bc))
        summary['comparisons'][f'round{repeat}'] = {
            'whole_slowdown': b['whole_ms']['mean'] / a['whole_ms']['mean'],
            'fluid_slowdown': b['fluid_ms']['mean'] / a['fluid_ms']['mean'],
            'iterations_ratio': b['iterations']['mean'] / a['iterations']['mean'],
            'incoming_volume_relative_change': b['incoming_volume'] / a['incoming_volume'] - 1,
            'upper_fraction_change': b['upper_fraction'] - a['upper_fraction'],
            'max_sampled_center_difference': max(math.hypot(x['x'] - y['x'], x['y'] - y['y']) for x, y in zip(ac, bc)),
            'different_edges_at_end': sum(x['edge'] != y['edge'] for x, y in zip(ac, bc) if x['frame'] == round(manifest['seconds'] * 60)),
        }
    for variant, count in [('cg', 4), ('simple', 5)]:
        assert f'{count} passed; 0 failed' in (root / f'{variant}-audit.log').read_text()
    audit = re.search(r'SIMPLE_AUDIT max_abs_Pz_minus_rhs=(\S+)', (root / 'simple-audit.log').read_text())
    summary['preconditioner_audit_max_abs_residual'] = float(audit[1])
    assert summary['preconditioner_audit_max_abs_residual'] < 1e-10
    (root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
