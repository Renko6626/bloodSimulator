#!/usr/bin/env python3
"""Report the cost/error tradeoff; do not assume loose tolerances are accepted."""
import argparse
import json
import math
from pathlib import Path

from analyze import ROOT, read, stats
from run import sha


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--results', type=Path, default=ROOT/'results/tolerance')
    args = parser.parse_args()
    result = args.results
    manifest = json.loads((result/'manifest.json').read_text())
    for p,h in manifest['sources_sha256'].items():
        assert sha(ROOT/p) == h, p
    summary = {}
    for run in manifest['runs']:
        assert run['exit_code'] == 0
        name = run['name']
        folder = result/name
        frames = read(folder/'frames.csv')
        assert len(frames) == 60
        assert [f['frame'] for f in frames] == list(range(60))
        fluxes = [list(map(float,line.split(',')[1:])) for line in (folder/'profile.log').read_text().splitlines() if line.startswith('FLUX,')]
        assert len(fluxes) == 120
        assert all(math.isfinite(v) and v > 0 for f in fluxes for v in f)
        reference = result/('strict-r'+name[-1])
        actual_cells = read(folder/'cells.csv')
        strict_cells = read(reference/'cells.csv')
        assert len(actual_cells) == len(strict_cells) == 60*72
        assert all((a['frame'],a['cell'])==(b['frame'],b['cell']) for a,b in zip(actual_cells,strict_cells))
        center = max(math.hypot(a['x']-b['x'],a['y']-b['y']) for a,b in zip(actual_cells,strict_cells))
        area = max(abs(a['area']/b['area']-1) for a,b in zip(actual_cells,strict_cells))
        actual_arrows, strict_arrows = read(folder/'arrows.csv'),read(reference/'arrows.csv')
        assert len(actual_arrows) == len(strict_arrows)
        assert all((a['frame'],a['arrow'],a['x'],a['y'])==(b['frame'],b['arrow'],b['x'],b['y']) for a,b in zip(actual_arrows,strict_arrows))
        arrow = max(math.hypot(a['vx']-b['vx'],a['vy']-b['vy']) for a,b in zip(actual_arrows,strict_arrows))
        speed = max(math.hypot(a['vx'],a['vy']) for a in strict_arrows)
        strict_fluxes = [list(map(float,line.split(',')[1:])) for line in (reference/'profile.log').read_text().splitlines() if line.startswith('FLUX,')]
        flux_change = max(abs(a[0]/b[0]-1) for a,b in zip(fluxes,strict_fluxes))
        summary[name] = {
            'tolerance':run['tolerance'],
            'whole_ms':stats([f['whole_ms'] for f in frames]),
            'fluid_ms':stats([f['fluid_ms'] for f in frames]),
            'iterations':stats([f['iterations'] for f in frames]),
            'inverse_calls_per_frame':sum(f['iterations'] for f in frames)/60+6,
            'max_frame_diagnostics':{k:max(abs(f[k]) for f in frames) for k in ['divergence','momentum','exchange','net_flux']},
            'max_substep_relative_flux_imbalance':max(abs(o-i)/((i+o)/2) for i,o in fluxes),
            'max_incoming_flow_relative_change_vs_strict':flux_change,
            'max_center_distance_vs_strict':center,
            'max_center_percent_diameter_vs_strict':center/1.4*100,
            'max_area_relative_change_vs_strict':area,
            'max_arrow_velocity_distance_vs_strict':arrow,
            'max_strict_arrow_speed':speed,
            'max_arrow_error_over_max_speed':arrow/speed,
        }
    (result/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))


if __name__ == '__main__':
    main()
