#!/usr/bin/env python3
"""Recompute phase costs, sampling counts and observable output equivalence."""
import argparse
import csv
import json
import math
from pathlib import Path
import statistics
import sys

ROOT=Path(__file__).resolve().parent
REPO=ROOT.parents[3]
sys.path.insert(0,str(ROOT.parent/'2026-10-26-stokes-sparse'))
from run import sha


def read(path):
    with path.open() as stream:
        rows=[{k:float(v) for k,v in r.items()} for r in csv.DictReader(stream)]
    assert rows and all(math.isfinite(v) for r in rows for v in r.values()),path
    return rows


def stats(values):
    ordered=sorted(values)
    return {'mean':statistics.mean(values),'p95':ordered[math.ceil(len(values)*.95)-1],'max':ordered[-1]}


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--results',type=Path,default=ROOT/'results')
    args=parser.parse_args()
    root=args.results
    manifest=json.loads((root/'manifest.json').read_text())
    assert len(manifest['runs'])==6 and all(r['exit_code']==0 for r in manifest['runs'])
    for p,h in manifest['sources_sha256'].items():
        assert sha(REPO/p)==h,p
    summary={'cases':{},'comparisons':{}}
    phase_fields=['network_ms','prepare_ms','robot_ms','advection_ms','fluid_ms','rapier_ms']
    reference=root/'baseline-r1'
    reference_frames=read(reference/'frames.csv')
    reference_fingerprints=(reference/'fingerprints.csv').read_bytes()
    semantic_keys=[k for k in reference_frames[0] if k not in ['whole_ms','fluid_ms']]
    expected_frames=round(manifest['seconds']*60)
    stage_data={}
    for run in manifest['runs']:
        name=run['name']
        folder=root/name
        frames=read(folder/'frames.csv')
        stages=read(folder/'stages.csv')
        assert len(frames)==len(stages)==expected_frames
        assert [r['frame'] for r in frames]==list(range(1,expected_frames+1))
        assert [r['frame'] for r in stages]==[r['frame'] for r in frames]
        assert all(all(a[k]==b[k] for k in semantic_keys) for a,b in zip(frames,reference_frames)),name
        assert (folder/'fingerprints.csv').read_bytes()==reference_fingerprints, name
        for f in ['cells.csv','arrows.csv','crossings.csv']:
            assert (folder/f).read_bytes()==(reference/f).read_bytes(),(name,f)
        assert max(r['max_area_error'] for r in frames)<.25
        assert max(r['max_wall_overshoot'] for r in frames)<.15
        assert max(r['exchange'] for r in frames)<1e-7
        case={'whole_ms':stats([r['whole_ms'] for r in frames]),
              'fluid_ms':stats([r['fluid_ms'] for r in frames]),
              'nonfluid_remainder_ms':stats([r['whole_ms']-r['fluid_ms'] for r in frames]),
              'active_cells':[min(r['active'] for r in frames),max(r['active'] for r in frames)],
              'max_diagnostics':{k:max(r[k] for r in frames) for k in ['divergence_peak','momentum_peak','exchange','max_area_error','max_wall_overshoot']},
              'observable_outputs_match':True}
        if run['profiling']:
            assert all(r['enabled']==1 for r in stages)
            case['stages']={k:stats([r[k] for r in stages]) for k in phase_fields}
            residual=[f['whole_ms']-sum(s[k] for k in phase_fields) for f,s in zip(frames,stages)]
            assert min(residual)>=-1e-8
            case['stages']['other_ms']=stats(residual)
            case['counts']={k:stats([r[k] for r in stages]) for k in ['analytic_samples','zero_drag_nodes','zero_drag_bodies']}
            stage_data[run['variant']]=stages
        else:
            assert all(r['enabled']==0 for r in stages)
        summary['cases'][name]=case
    baseline,candidate=stage_data['baseline'],stage_data['skip']
    assert all(a['zero_drag_nodes']==b['zero_drag_nodes'] for a,b in zip(baseline,candidate))
    assert all(a['analytic_samples']-b['analytic_samples']==a['zero_drag_nodes'] for a,b in zip(baseline,candidate))
    summary['comparisons']['profile_sampling_reduction_fraction']=1-sum(r['analytic_samples'] for r in candidate)/sum(r['analytic_samples'] for r in baseline)
    for repeat in [1,2]:
        a=summary['cases'][f'baseline-r{repeat}']
        b=summary['cases'][f'skip-r{repeat}']
        summary['comparisons'][f'main-r{repeat}']={
            'whole_mean_saved_ms':a['whole_ms']['mean']-b['whole_ms']['mean'],
            'nonfluid_mean_saved_ms':a['nonfluid_remainder_ms']['mean']-b['nonfluid_remainder_ms']['mean']}
    for variant in ['baseline','skip']:
        assert '1 passed; 0 failed' in (root/f'{variant}-cell-test.log').read_text()
    (root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))


if __name__=='__main__':
    main()
