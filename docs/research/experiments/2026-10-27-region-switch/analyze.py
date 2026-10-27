#!/usr/bin/env python3
"""Analyze the actual production driving probe using archived source hashes."""
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


def stats(values):
    values = sorted(values)
    assert values
    return {'mean': statistics.mean(values), 'p95': values[math.ceil(len(values) * .95) - 1], 'max': values[-1]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--results', type=Path, default=ROOT / 'results')
    root = parser.parse_args().results
    manifest = json.loads((root / 'manifest.json').read_text())
    assert manifest['exit_code'] == 0
    for name, expected in manifest['sources_sha256'].items():
        assert hashlib.sha256((root / 'sources' / name).read_bytes()).hexdigest() == expected, name
    with (root / 'drive/frames.csv').open() as stream:
        rows = [{k: float(v) for k, v in r.items()} for r in csv.DictReader(stream)]
    assert len(rows) == round(manifest['seconds'] * 60)
    assert all(math.isfinite(v) for r in rows for v in r.values())
    assert [r['frame'] for r in rows] == list(range(1, len(rows) + 1))
    switches = [r for r in rows if r['switches'] > 0]
    assert sum(r['switches'] for r in rows) >= 2
    assert len({(r['min_x'], r['min_y']) for r in rows}) == 2
    for key, threshold in [('area_error', .25), ('wall_overshoot', .15), ('exchange', 1e-7),
                           ('divergence', 1e-8), ('momentum', 1e-8)]:
        assert max(r[key] for r in rows) < threshold, key
    summary = {
        'frames': len(rows), 'switches': sum(r['switches'] for r in rows),
        'initialization_ms': float(re.search(r'init_ms=([\d.]+)', (root / 'run.log').read_text())[1]),
        'max_process_rss_kib': int(re.search(r'MAX_RSS_KIB,(\d+)', (root / 'stderr.log').read_text())[1]),
        'whole_ms': stats([r['whole_ms'] for r in rows]),
        'fluid_ms': stats([r['fluid_ms'] for r in rows]),
        'switch_frame_whole_ms': stats([r['whole_ms'] for r in switches]),
        'handoff_ms': stats([r['handoff_ms'] for r in switches]),
        'handoff_local_velocity_delta': stats([r['velocity_delta'] for r in switches]),
        'max_frame_diagnostics': {k: max(r[k] for r in rows) for k in
                                  ['divergence', 'momentum', 'exchange', 'area_error', 'wall_overshoot']},
        'switch_events': switches,
        'current_sources_match': all((REPO / n).exists() and hashlib.sha256((REPO / n).read_bytes()).hexdigest() == h
                                     for n, h in manifest['sources_sha256'].items()),
    }
    (root / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
    print(json.dumps(summary, indent=2))


if __name__ == '__main__':
    main()
