#!/usr/bin/env python3
"""Audit same-candidate observations and report nested/parallel timing correctly."""
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
        rows=[{k:float(v) for k,v in row.items()} for row in csv.DictReader(stream)]
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
    assert len(manifest['runs'])==5 and all(r['exit_code']==0 for r in manifest['runs'])
    for p,h in manifest['sources_sha256'].items():
        assert hashlib.sha256((REPO/p).read_bytes()).hexdigest()==h,p
    expected=round(manifest['seconds']*60)
    reference=root/'control-r1'
    reference_frames=read(reference/'frames.csv')
    keys=[k for k in reference_frames[0] if k not in ['whole_ms','fluid_ms']]
    # First 30 seconds must reproduce the previously accepted six-sample case.
    old=ROOT.parent/'2026-10-26-runtime-hotpaths/results/six-sample/six-r1'
    if manifest['seconds']==30:
        assert (reference/'fingerprints.csv').read_bytes()==(old/'fingerprints.csv').read_bytes()
    summary={'cases':{},'notes':[
        'geometry and fluid are separate instrumented trajectories, not additive phases of one run',
        'schur/grid/inverse/measure spans nest; do not sum all rows',
        'factor_*_work sums elapsed work across two parallel tasks; not frame wall time',
        'native Rapier timers are a separate build with only the profiler feature enabled',
        'zero native sub-timers may be uninstrumented in the parallel implementation']}
    for run in manifest['runs']:
        folder=root/run['name'];frames=read(folder/'frames.csv');deep=read(folder/'deep.csv');stages=read(folder/'stages.csv')
        assert len(frames)==len(deep)==len(stages)==expected
        assert [r['frame'] for r in frames]==list(range(1,expected+1))
        assert [r['frame'] for r in deep]==[r['frame'] for r in frames]
        assert all(all(a[k]==b[k] for k in keys) for a,b in zip(frames,reference_frames)),run['name']
        for name in ['fingerprints.csv','cells.csv','arrows.csv','crossings.csv']:
            assert (folder/name).read_bytes()==(reference/name).read_bytes(),(run['name'],name)
        assert max(r['max_area_error'] for r in frames)<.25
        assert max(r['max_wall_overshoot'] for r in frames)<.15
        assert max(r['exchange'] for r in frames)<1e-7
        phase_fields=['network_ms','prepare_ms','robot_ms','advection_ms','fluid_ms','rapier_ms']
        case={'whole_ms':stats([r['whole_ms'] for r in frames]),
              'fluid_ms':stats([r['fluid_ms'] for r in frames]),
              'stages':{k:stats([r[k] for r in stages]) for k in phase_fields},
              'iterations_mean':statistics.mean(r['iterations'] for r in frames),
              'analytic_samples_mean':statistics.mean(r['analytic_samples'] for r in stages),
              'observations_match':True,
              'metrics_ms':{},'metric_calls':{}}
        names=[k[:-3] for k in deep[0] if k.endswith('_ns')]
        for name in names:
            if sum(r[name+'_calls'] for r in deep):
                case['metrics_ms'][name]=stats([r[name+'_ns']/1e6 for r in deep])
                case['metric_calls'][name]=statistics.mean(r[name+'_calls'] for r in deep)
        if run['mode']=='geometry':
            # These functions do not call one another; junction_mix overlaps
            # project_edge/edge_length and is deliberately excluded here.
            disjoint=['nearest_edge','signed_distance','project_edge','edge_length','junction_search']
            rest=[(r['analytic_total_ns']-sum(r[k+'_ns'] for k in disjoint))/1e6 for r in deep]
            assert min(rest)>=-1e-8
            case['analytic_other_ms']=stats(rest)
            assert all(r['nearest_edge_calls']==r['signed_distance_calls']==r['analytic_total_calls'] for r in deep)
        if run['mode']=='fluid':
            assert all(r['grid_solve_calls']==2 for r in deep)
            assert all(r['velocity_inverse_calls']==f['iterations']+6 for r,f in zip(deep,frames))
            assert all(r['factor_total_work_calls']==2*r['velocity_inverse_calls'] for r in deep)
            outside=[(r['velocity_inverse_ns']-r['max_component_per_inverse_ns'])/1e6 for r in deep]
            assert min(outside)>=-1e-8
            case['inverse_outside_max_component_ms']=stats(outside)
            case['inverse_fraction_of_grid_solve']=sum(r['velocity_inverse_ns'] for r in deep)/sum(r['grid_solve_ns'] for r in deep)
            case['inverse_mean_ms_per_call']=sum(r['velocity_inverse_ns'] for r in deep)/sum(r['velocity_inverse_calls'] for r in deep)/1e6
        if run['mode']=='rapier':
            assert any(r['rapier_solver_ns']>0 for r in deep)
            assert all(r['rapier_solver_calls']==2 for r in deep)
        if run['mode']=='none':
            assert all(all(v==0 for k,v in r.items() if k!='frame') for r in deep)
        summary['cases'][run['name']]=case
    (root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))


if __name__=='__main__':main()
