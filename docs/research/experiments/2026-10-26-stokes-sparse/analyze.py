#!/usr/bin/env python3
"""Recompute matched performance and numerical checks from saved raw output."""
import argparse
import csv
import hashlib
import json
import math
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parent


def read(path):
    with path.open() as stream:
        rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(stream)]
    assert rows and all(math.isfinite(v) for r in rows for v in r.values()), path
    return rows


def stats(values):
    ordered = sorted(values)
    return {'mean': statistics.mean(values), 'p95': ordered[math.ceil(len(values)*.95)-1], 'max': ordered[-1]}


def compare(a, b, fields):
    assert len(a) == len(b)
    keys = [k for k in a[0] if k not in fields]
    assert all(all(x[k] == y[k] for k in keys) for x, y in zip(a, b))
    return {k: max(abs(x[k]-y[k]) for x, y in zip(a, b)) for k in fields}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--results', type=Path, default=ROOT/'results')
    args = parser.parse_args()
    result = args.results
    manifest = json.loads((result/'manifest.json').read_text())
    for p, expected in manifest['sources_sha256'].items():
        assert hashlib.sha256((ROOT/p).read_bytes()).hexdigest() == expected, p
    summary = {'cases': {}, 'comparisons': {}}
    cells, arrows = {}, {}
    for run in manifest['runs']:
        assert run['exit_code'] == 0
        name = run['name']
        folder = result/name
        frames = read(folder/'frames.csv')
        assert len(frames) == 60 and [r['frame'] for r in frames] == list(range(60))
        assert max(r['divergence'] for r in frames) < 1e-8
        assert max(r['momentum'] for r in frames) < 1e-8
        assert max(r['exchange'] for r in frames) < 1e-7
        assert max(abs(r['net_flux']) for r in frames) < 1e-7
        case = {key: stats([r[key] for r in frames]) for key in ['whole_ms','fluid_ms','iterations']}
        case['max_diagnostics'] = {k: max(abs(r[k]) for r in frames) for k in ['divergence','momentum','exchange','net_flux']}
        case['active_cells'] = [min(r['active'] for r in frames), max(r['active'] for r in frames)]
        factor = []
        profile = []
        for line in (folder/'profile.log').read_text().splitlines():
            parts = line.split(',')
            if parts[0] == 'FACTOR':
                axis, n, entries, payload, ns = map(int, parts[1:])
                factor.append({'axis':axis,'size':n,'entries':entries,'payload_bytes':payload,'setup_ms':ns/1e6})
            elif parts[0] == 'PROFILE':
                profile.append(list(map(int,parts[1:])))
        assert len(factor) == 2
        case['factor'] = sorted(factor, key=lambda f:f['axis'])
        case['factor_entries'] = sum(f['entries'] for f in factor)
        case['factor_payload_bytes'] = sum(f['payload_bytes'] for f in factor)
        log = (folder/'run.log').read_text().splitlines()
        case['initialization_ms_rounded'] = float(log[0].split('init=')[1].split('ms')[0])
        if run['profiling']:
            assert len(profile) == 121  # background solve, then 60*2 dynamic solves
            profile = profile[1:]
            assert all(calls == iters+3 for iters,calls,_,_ in profile)
            inverse_ns = sum(p[3] for p in profile)
            calls = sum(p[1] for p in profile)
            case['profile'] = {'inverse_calls_per_frame':calls/60,
                               'inverse_ms_per_frame':inverse_ns/60e6,
                               'inverse_ms_per_call':inverse_ns/calls/1e6,
                               'grid_solve_ms_per_frame':sum(p[2] for p in profile)/60e6}
        else:
            assert not profile
        summary['cases'][name] = case
        cells[name] = read(folder/'cells.csv')
        arrows[name] = read(folder/'arrows.csv')
    for a, b in [('band-r1','sparse-r1'),('band-r2','sparse-r2'),('band-r1','band-r2'),('sparse-r1','sparse-r2')]:
        cell = compare(cells[a], cells[b], ['x','y','area'])
        # Arrow positions must be identical; only compare their velocities.
        arrow = compare(arrows[a], arrows[b], ['vx','vy'])
        summary['comparisons'][a+'__'+b] = {'cells_max_abs':cell,'arrows_max_abs':arrow}
        assert max(cell.values()) < 1e-5
        assert max(arrow.values()) < 1e-5
    for variant in ['band','sparse']:
        assert '4 passed; 0 failed' in (result/f'{variant}-tests.log').read_text()
        assert 'COMPARE velocity_max_abs=' in (result/f'{variant}-tests.log').read_text()
    (result/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))


if __name__ == '__main__':
    main()
