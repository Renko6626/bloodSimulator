#!/usr/bin/env python3
"""Summarize observed functional diagnostics; no product acceptance thresholds."""
import ast
import json
import re
from pathlib import Path

root = Path(__file__).resolve().parent
spectrum = json.loads((root / 'results/spectrum.json').read_text())
summary = {'spectra': {}, 'cases': {}}
for profile in ['fine', 'coarse']:
    for closure in ['velocity', 'fd', 'fd_rho1']:
        rows = [r for r in spectrum if r['profile'] == profile and r['closure'] == closure]
        assert len(rows) == 17
        summary['spectra'][f'{profile}-{closure}'] = {
            'radius': max(r['radius'] for r in rows),
            'nonzero_ky_radius': max(r['radius'] for r in rows if r['k']),
            'max_k': max(rows, key=lambda r: r['radius'])['k'],
        }
for case in ['straight-fine', 'straight-coarse', 'fork-fine', 'fork-coarse', 'forced-moments']:
    lines = (root / f'results/{case}.log').read_text().splitlines()
    measures = [line for line in lines if line.startswith('measure ')]
    fields = [line for line in lines if line.startswith('frame=')]
    assert lines[-1] == 'COMPLETE' and len(measures) == (4 if case == 'forced-moments' else 160)
    values = lambda key: [float(re.search(rf'{key}=([^ ,]+)', line)[1]) for line in measures]
    flux_errors = []
    for line in measures:
        flux = ast.literal_eval(re.search(r'flux=(\[[^]]+\])', line)[1])
        target_match = re.search(r'targets=(\[[^]]+\])', line)
        if target_match:
            target = ast.literal_eval(target_match[1])
        else:
            q = float(re.search(r'target=([^ ]+)', line)[1]); target = [-q, q]
        flux_errors.extend(abs(a-b) for a,b in zip(flux, target))
    summary['cases'][case] = {
        'frames': len(measures),
        'rho_min': min(float(re.search(r'lo=([^ ]+)', line)[1]) for line in fields),
        'rho_max': max(float(re.search(r'hi=([^ ]+)', line)[1]) for line in fields),
        'max_lattice_speed': max(float(re.search(r'ulat=([^ ]+)', line)[1]) for line in fields),
        'max_velocity_error_world': max(values('velocity_error')),
        'max_flux_error_world': max(flux_errors),
        'mass_rel_min': min(values('mass_rel')),
        'mass_rel_max': max(values('mass_rel')),
        'mass_rel_final': values('mass_rel')[-1],
    }
print(json.dumps(summary, indent=2))
(root / 'results/summary.json').write_text(json.dumps(summary, indent=2)+'\n')
