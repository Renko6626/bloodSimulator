#!/usr/bin/env python3
"""Cost and observable error of 6 samples; never claim exact 12-sample replay."""
import argparse
from collections import Counter
import csv
import hashlib
import json
import math
from pathlib import Path
import statistics

ROOT=Path(__file__).resolve().parent
REPO=ROOT.parents[3]


def read(path):
    with path.open() as stream:
        rows=[{k:float(v) for k,v in row.items()} for row in csv.DictReader(stream)]
    assert all(math.isfinite(v) for r in rows for v in r.values()),path
    return rows


def stats(values):
    values=sorted(values)
    return {'mean':statistics.mean(values),'p95':values[math.ceil(len(values)*.95)-1],'max':values[-1]}


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--results',type=Path,default=ROOT/'results/six-sample')
    args=parser.parse_args()
    root=args.results
    manifest=json.loads((root/'manifest.json').read_text())
    assert len(manifest['runs'])==6 and all(r['exit_code']==0 for r in manifest['runs'])
    for p,h in manifest['sources_sha256'].items():
        assert hashlib.sha256((REPO/p).read_bytes()).hexdigest()==h,p
    summary={'cases':{},'comparisons':{}}
    for run in manifest['runs']:
        name=run['name'];folder=root/name
        frames=read(folder/'frames.csv')
        assert len(frames)==round(manifest['seconds']*60)
        assert [r['frame'] for r in frames]==list(range(1,len(frames)+1))
        assert max(r['max_area_error'] for r in frames)<.25
        assert max(r['max_wall_overshoot'] for r in frames)<.15
        assert max(r['exchange'] for r in frames)<1e-7
        cells=read(folder/'cells.csv')
        final=[c for c in cells if c['frame']==len(frames)]
        assert len(final)==72
        counts=Counter((int(r['from']),int(r['to'])) for r in read(folder/'crossings.csv'))
        total_in=sum(r['in_volume'] for r in frames)
        upper=sum(r['upper_volume'] for r in frames)
        lower=sum(r['lower_volume'] for r in frames)
        case={'whole_ms':stats([r['whole_ms'] for r in frames]),
              'fluid_ms':stats([r['fluid_ms'] for r in frames]),
              'iterations':stats([r['iterations'] for r in frames]),
              'nonfluid_ms':stats([r['whole_ms']-r['fluid_ms'] for r in frames]),
              'active_cells':[min(r['active'] for r in frames),max(r['active'] for r in frames)],
              'max_diagnostics':{k:max(r[k] for r in frames) for k in ['contacts','divergence_peak','momentum_peak','exchange','max_area_error','max_wall_overshoot','grid_speed_peak','force_peak','flux_relative_peak']},
              'fork_crossings':[counts.get((0,1),0),counts.get((0,4),0)],
              'mean_cell_distance':statistics.mean(c['distance'] for c in final),
              'incoming_volume':total_in,
              'upper_outlet_fraction':upper/(upper+lower)}
        if run['profiling']:
            stages=read(folder/'stages.csv')
            assert len(stages)==len(frames) and all(r['enabled']==1 for r in stages)
            divisor=2 if run['variant']=='six' else 1
            assert all(r['analytic_samples']==(72*12*2-r['zero_drag_nodes'])/divisor for r in stages)
            keys=['network_ms','prepare_ms','robot_ms','advection_ms','fluid_ms','rapier_ms']
            case['stages']={k:stats([r[k] for r in stages]) for k in keys}
            case['analytic_samples']=stats([r['analytic_samples'] for r in stages])
        summary['cases'][name]=case
        reference=root/(run['variant']+'-r1')
        reference_frames=read(reference/'frames.csv')
        semantic_keys=[k for k in frames[0] if k not in ['whole_ms','fluid_ms']]
        assert all(all(a[k]==b[k] for k in semantic_keys) for a,b in zip(frames,reference_frames)),name
        assert (folder/'fingerprints.csv').read_bytes()==(reference/'fingerprints.csv').read_bytes(),name
        for f in ['cells.csv','arrows.csv','crossings.csv']:
            assert (folder/f).read_bytes()==(reference/f).read_bytes(),(name,f)
    for repeat in [1,2]:
        a=summary['cases'][f'twelve-r{repeat}'];b=summary['cases'][f'six-r{repeat}']
        ac=read(root/f'twelve-r{repeat}'/'cells.csv');bc=read(root/f'six-r{repeat}'/'cells.csv')
        assert len(ac)==len(bc) and all((x['frame'],x['cell'])==(y['frame'],y['cell']) for x,y in zip(ac,bc))
        final=[(x,y) for x,y in zip(ac,bc) if x['frame']==round(manifest['seconds']*60)]
        summary['comparisons'][f'round{repeat}']={
            'whole_mean_saved_ms':a['whole_ms']['mean']-b['whole_ms']['mean'],
            'nonfluid_mean_saved_ms':a['nonfluid_ms']['mean']-b['nonfluid_ms']['mean'],
            'mean_cell_distance_relative_change':b['mean_cell_distance']/a['mean_cell_distance']-1,
            'incoming_volume_relative_change':b['incoming_volume']/a['incoming_volume']-1,
            'upper_outlet_fraction_change':b['upper_outlet_fraction']-a['upper_outlet_fraction'],
            'max_sampled_cell_position_difference':max(math.hypot(x['x']-y['x'],x['y']-y['y']) for x,y in zip(ac,bc)),
            'different_edges_at_end':sum(x['edge']!=y['edge'] for x,y in final)}
    for variant in ['twelve','six']:
        log=(root/f'{variant}-checks.log').read_text()
        assert '2 passed; 0 failed' in log and 'uniform_flow_keeps_full_ring_response ... ok' in log
    (root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))


if __name__=='__main__':
    main()
