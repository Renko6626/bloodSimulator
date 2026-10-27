#!/usr/bin/env python3
"""Audit behavior preservation and compare equivalent geometry-query work."""
import argparse
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
        rows=[{k:float(v) for k,v in r.items()} for r in csv.DictReader(stream)]
    assert rows and all(math.isfinite(v) for r in rows for v in r.values()),path
    return rows


def stats(values):
    values=sorted(values)
    return {'mean':statistics.mean(values),'p95':values[math.ceil(len(values)*.95)-1],'max':values[-1]}


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--results',type=Path,default=ROOT/'results')
    args=parser.parse_args();root=args.results
    manifest=json.loads((root/'manifest.json').read_text())
    assert len(manifest['runs'])==6 and all(r['exit_code']==0 for r in manifest['runs'])
    for p,h in manifest['sources_sha256'].items():
        assert hashlib.sha256((REPO/p).read_bytes()).hexdigest()==h,p
    baseline=root/'before-r1'
    reference=read(baseline/'frames.csv')
    semantic=[k for k in reference[0] if k not in ['whole_ms','fluid_ms']]
    old=ROOT.parent/'2026-10-26-runtime-hotpaths/results/six-sample/six-r1/fingerprints.csv'
    if manifest['seconds']==30:
        assert (baseline/'fingerprints.csv').read_bytes()==old.read_bytes()
    summary={'cases':{},'comparisons':{}}
    for run in manifest['runs']:
        name=run['name'];folder=root/name
        frames=read(folder/'frames.csv');stages=read(folder/'stages.csv');deep=read(folder/'deep.csv')
        assert len(frames)==len(stages)==len(deep)==round(manifest['seconds']*60)
        assert [r['frame'] for r in frames]==list(range(1,len(frames)+1))
        assert all(all(a[k]==b[k] for k in semantic) for a,b in zip(frames,reference)),name
        for f in ['fingerprints.csv','cells.csv','arrows.csv','crossings.csv']:
            assert (folder/f).read_bytes()==(baseline/f).read_bytes(),(name,f)
        assert max(r['max_area_error'] for r in frames)<.25
        assert max(r['max_wall_overshoot'] for r in frames)<.15
        assert max(r['exchange'] for r in frames)<1e-7
        fields=['network_ms','prepare_ms','robot_ms','advection_ms','fluid_ms','rapier_ms']
        case={'whole_ms':stats([r['whole_ms'] for r in frames]),
              'fluid_ms':stats([r['fluid_ms'] for r in frames]),
              'stages':{k:stats([r[k] for r in stages]) for k in fields},
              'observations_match':True}
        if run['mode']=='geometry':
            case['geometry_ms']={k:stats([r[k+'_ns']/1e6 for r in deep]) for k in ['analytic_total','nearest_edge','signed_distance','project_edge','junction_search','junction_mix','edge_length']}
            case['geometry_calls']={k:statistics.mean(r[k+'_calls'] for r in deep) for k in ['analytic_total','nearest_edge','signed_distance']}
            if run['variant']=='reuse':
                assert all(r['signed_distance_calls']==0 for r in deep)
            else:
                assert all(r['signed_distance_calls']==r['analytic_total_calls'] for r in deep)
            assert all(r['nearest_edge_calls']==r['analytic_total_calls'] for r in deep)
        summary['cases'][name]=case
    for repeat in [1,2]:
        a=summary['cases'][f'before-r{repeat}'];b=summary['cases'][f'reuse-r{repeat}']
        summary['comparisons'][f'round{repeat}']={
            'advection_mean_saved_ms':a['stages']['advection_ms']['mean']-b['stages']['advection_ms']['mean'],
            'whole_mean_saved_ms':a['whole_ms']['mean']-b['whole_ms']['mean']}
    for variant in ['before','reuse']:
        log=(root/f'{variant}-audit.log').read_text()
        assert '1 passed; 0 failed' in log
        assert 'max_velocity_error=0.00000000000000000e0' in log
    (root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))


if __name__=='__main__':main()
