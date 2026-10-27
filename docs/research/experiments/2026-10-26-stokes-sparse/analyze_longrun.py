#!/usr/bin/env python3
"""Audit a completed pair; quantify drift and macroscopic behavior separately."""
import argparse
from collections import Counter, defaultdict
import csv
import json
import math
from pathlib import Path
import statistics

from run import ROOT, sha


def read(path):
    with path.open() as stream:
        rows=[{k:float(v) for k,v in r.items()} for r in csv.DictReader(stream)]
    assert all(math.isfinite(v) for r in rows for v in r.values()),path
    return rows


def quantiles(values):
    values=sorted(values)
    return {'mean':statistics.mean(values),'median':statistics.median(values),
            'p95':values[math.ceil(len(values)*.95)-1],'max':values[-1]}


def continuity(rows):
    incoming=sum(r['in_volume'] for r in rows)
    outgoing=sum(r['out_volume'] for r in rows)
    absolute=sum(r['abs_net_volume'] for r in rows)
    upper=sum(r['upper_volume'] for r in rows)
    lower=sum(r['lower_volume'] for r in rows)
    assert incoming>0 and upper+lower>0
    return {'incoming_volume':incoming,'outgoing_volume':outgoing,
            'signed_defect':outgoing-incoming,'absolute_defect':absolute,
            'signed_defect_over_incoming':(outgoing-incoming)/incoming,
            'absolute_defect_over_incoming':absolute/incoming,
            'signed_defect_over_window_area':(outgoing-incoming)/(2484*.25**2),
            'upper_outlet_fraction':upper/(upper+lower)}


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--results',type=Path,default=ROOT/'results/longrun')
    args=parser.parse_args()
    root=args.results
    manifest=json.loads((root/'manifest.json').read_text())
    assert len(manifest['runs'])==2 and all(r['exit_code']==0 for r in manifest['runs'])
    for p,h in manifest['sources_sha256'].items():
        assert sha(ROOT/p)==h,p
    frame_data={}
    cell_data={}
    arrow_data={}
    event_data={}
    result={'cases':{},'comparisons':{}}
    for label in ['loose','strict']:
        folder=root/label
        frames=read(folder/'frames.csv')
        steps=round(manifest['seconds']/(1/60))
        assert len(frames)==steps
        assert [r['frame'] for r in frames]==list(range(1,steps+1))
        # Existing mechanical/impulse bounds; do not relax the old pressure
        # residual test and then claim the low-accuracy field passed it.
        assert max(r['max_area_error'] for r in frames)<.25
        assert max(r['max_wall_overshoot'] for r in frames)<.15
        assert max(r['exchange'] for r in frames)<1e-7
        frame_data[label]={r['frame']:r for r in frames}
        cells=read(folder/'cells.csv')
        grouped=defaultdict(list)
        for c in cells:
            grouped[c['frame']].append(c)
        assert all(len(cs)==72 for cs in grouped.values())
        cell_data[label]=grouped
        arrow_data[label]=read(folder/'arrows.csv')
        events=read(folder/'crossings.csv')
        event_data[label]=events
        case={
            'completed_frames':len(frames),'simulation_seconds':frames[-1]['time'],
            'whole_ms':quantiles([r['whole_ms'] for r in frames]),
            'fluid_ms':quantiles([r['fluid_ms'] for r in frames]),
            'iterations':quantiles([r['iterations'] for r in frames]),
            'active_cells':[min(r['active'] for r in frames),max(r['active'] for r in frames)],
            'maxima':{k:max(abs(r[k]) for r in frames) for k in ['contacts','divergence_peak','point_divergence_peak','momentum_peak','exchange','flux_relative_peak','grid_speed_peak','force_peak','max_area_error','max_wall_overshoot','max_center_speed']},
            'continuity':continuity(frames),
            'crossings':{f'{int(a)}->{int(b)}':v for (a,b),v in sorted(Counter((r['from'],r['to']) for r in events).items())},
            'final_mean_cell_distance':statistics.mean(c['distance'] for c in grouped[steps]),
            'time_blocks':[]}
        for start in range(0,steps,600):
            block=frames[start:start+600]
            case['time_blocks'].append({'end_seconds':block[-1]['time'],
                'mean_speed':statistics.mean(r['mean_center_speed'] for r in block),
                'max_grid_speed':max(r['grid_speed_peak'] for r in block),
                'max_force':max(r['force_peak'] for r in block),
                'max_area_error':max(r['max_area_error'] for r in block),
                'continuity':continuity(frames[:start+len(block)])})
        result['cases'][label]=case
    assert cell_data['strict'].keys()==cell_data['loose'].keys()
    samples=[]
    all_errors=[]
    max_area_relative=0.0
    first_pixel=None
    for frame in sorted(cell_data['strict']):
        strict,loose=cell_data['strict'][frame],cell_data['loose'][frame]
        assert [c['cell'] for c in strict]==[c['cell'] for c in loose]
        errors=[math.hypot(a['x']-b['x'],a['y']-b['y']) for a,b in zip(strict,loose)]
        all_errors.extend(errors)
        max_area_relative=max(max_area_relative,max(abs(a['area']/b['area']-1) for a,b in zip(loose,strict)))
        if frame:
            s=frame_data['strict'][frame]
            l=frame_data['loose'][frame]
            dx=l['robot_x']-s['robot_x']
            dy=l['robot_y']-s['robot_y']
            camera_errors=[math.hypot((b['x']-a['x'])-dx,(b['y']-a['y'])-dy)*24 for a,b in zip(strict,loose)]
        else:
            camera_errors=[0.0]*len(errors)
        sample={'frame':frame,'time':frame/60,'position_error':quantiles(errors),
                'max_world_error_pixels_at_24':max(errors)*24,
                'max_camera_relative_error_pixels_at_24':max(camera_errors),
                'different_edges':sum(a['edge']!=b['edge'] for a,b in zip(strict,loose))}
        samples.append(sample)
        if first_pixel is None and max(camera_errors)>=1:
            first_pixel=sample['time']
    result['comparisons']['cell_samples']={
        'sample_count':len(samples),'all_position_errors':quantiles(all_errors),
        'max_center_percent_diameter':max(all_errors)/1.4*100,
        'max_area_relative_difference':max_area_relative,
        'first_sample_at_least_one_camera_relative_pixel':first_pixel,
        'samples':samples}
    a,b=arrow_data['strict'],arrow_data['loose']
    assert len(a)==len(b)
    assert all((x['frame'],x['arrow'],x['x'],x['y'])==(y['frame'],y['arrow'],y['x'],y['y']) for x,y in zip(a,b))
    errors=[math.hypot(x['vx']-y['vx'],x['vy']-y['vy']) for x,y in zip(a,b)]
    result['comparisons']['arrows']={'max_velocity_difference':max(errors),'rms_velocity_difference':math.sqrt(sum(e*e for e in errors)/len(errors))}
    strict,loose=result['cases']['strict'],result['cases']['loose']
    result['comparisons']['macroscopic']={
        'incoming_volume_relative_change':loose['continuity']['incoming_volume']/strict['continuity']['incoming_volume']-1,
        'upper_outlet_fraction_difference':loose['continuity']['upper_outlet_fraction']-strict['continuity']['upper_outlet_fraction'],
        'mean_cell_travel_relative_change':loose['final_mean_cell_distance']/strict['final_mean_cell_distance']-1,
        'fork_crossings_strict':[strict['crossings'].get('0->1',0),strict['crossings'].get('0->4',0)],
        'fork_crossings_loose':[loose['crossings'].get('0->1',0),loose['crossings'].get('0->4',0)]}
    replay_folder=root/'replay-strict'
    if replay_folder.exists():
        replay_manifest=json.loads((replay_folder/'manifest.json').read_text())
        assert replay_manifest['exit_code']==0
        assert replay_manifest['binary_sha256']==manifest['binary_sha256']['strict']
        assert replay_manifest['script_sha256']==sha(ROOT/'replay_longrun.py')
        replay_frames=read(replay_folder/'frames.csv')
        assert len(replay_frames)==round(replay_manifest['seconds']*60)
        keys=[k for k in replay_frames[0] if k not in ['whole_ms','fluid_ms']]
        values_equal=all(all(r[k]==frame_data['strict'][r['frame']][k] for k in keys) for r in replay_frames)
        replay_cells=read(replay_folder/'cells.csv')
        max_error=0.0
        cell_values_equal=True
        for c in replay_cells:
            s=cell_data['strict'][c['frame']][int(c['cell'])]
            assert s['cell']==c['cell']
            cell_values_equal &= c==s
            max_error=max(max_error,math.hypot(c['x']-s['x'],c['y']-s['y']))
        result['comparisons']['strict_replay']={
            'seconds':replay_manifest['seconds'],
            'all_nontiming_frame_values_equal':values_equal,
            'all_sampled_cell_values_equal':cell_values_equal,
            'max_sampled_cell_position_difference':max_error}
    (root/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:v for k,v in result.items() if k=='cases'},indent=2))
    print(json.dumps(result['comparisons']['macroscopic'],indent=2))
    print('Cell position samples:',result['comparisons']['cell_samples']['all_position_errors'])


if __name__=='__main__':
    main()
