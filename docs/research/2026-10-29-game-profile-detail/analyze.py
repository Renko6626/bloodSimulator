#!/usr/bin/env python3
"""Close wall-clock intervals and summarize overlapping diagnostic timers."""
import argparse, csv, json, math, re
from collections import Counter
from pathlib import Path

def rows(p):
    with p.open() as f:
        return list(csv.DictReader(f))

def stats(xs):
    xs = sorted(xs)
    if not xs:
        return None
    return dict(n=len(xs), mean=sum(xs)/len(xs),
                p50=xs[math.ceil(.5*len(xs))-1], p95=xs[math.ceil(.95*len(xs))-1],
                p99=xs[math.ceil(.99*len(xs))-1], min=xs[0], max=xs[-1])

def log_frames(p):
    result, current = {}, None
    for line in p.read_text().splitlines():
        if not line.startswith('PROFILE_'):
            continue
        kind = line.split()[0][8:]
        fields = dict(re.findall(r'(\w+)=([^\s]+)', line))
        if kind == 'BEGIN':
            current = int(fields['frame'])
            result[current] = {}
        if kind == 'SNAPSHOT':
            target = int(fields['frame'])-1
            if target in result:
                result[target].setdefault(kind, []).append(fields)
        elif current is not None:
            result[current].setdefault(kind, []).append(fields)
            if kind=='CACHE':
                # Migration may read a field before either mechanical batch.
                # Only refresh_field emits CACHE; its preceding read is unambiguous.
                result[current].setdefault('CACHE_READ',[]).append(result[current]['READBACK'][-1])
    return result

def analyze(root):
    output, trajectories = {}, {}
    monitor=rows(root/'gpu-timeline.csv')
    names=['production', 'cpu', 'render', 'gpu', 'phases']
    for name in names:
        assert (root/f'{name}.exit.txt').read_text().strip() == '0', name
        assert (root/f'{name}.csv.status.txt').read_text().strip() == 'COMPLETE', name
        data = rows(root/f'{name}.csv')
        assert len(data) == 3720, (name, len(data))
        assert all(r['finite']=='true' and int(r['lattice_steps'])==14 for r in data)
        assert all(abs(float(r['detailed_cells'])+float(r['abstract_cells'])-float(r['total_cells']))<1e-6 for r in data)
        render = rows(root/f'{name}.csv.render.csv')
        selected = [r for r in data if r['phase']=='measure']
        intervals = [r for prev, r in zip(render, render[1:])
                     if 120 <= int(prev['physics_frame']) < 3720 and 120 <= int(r['physics_frame']) < 3720]
        assert len(selected)==3600 and len(intervals)==3599, (name,len(selected),len(intervals))
        item = {'interval_ms':stats([float(r['render_interval_ms']) for r in intervals]),
                'draw_calls':stats([float(r['draw_calls_previous_frame']) for r in intervals]),
                'region_switches':int(data[-1]['region_switches']),
                'detailed_range':[min(int(r['detailed_cells']) for r in data),max(int(r['detailed_cells']) for r in data)],
                'spawned':sum(int(r['spawned']) for r in data), 'retired':sum(int(r['retired']) for r in data)}
        start=(root/f'{name}.started.txt').read_text().strip().removesuffix('Z')
        end=(root/f'{name}.finished.txt').read_text().strip().removesuffix('Z')
        this_monitor=rows(root/'phases-gpu-timeline.csv') if name=='phases' else monitor
        gpu_rows=[{k.strip():v.strip() for k,v in r.items()} for r in this_monitor
                  if start<=r['timestamp'].replace('/','-').replace(' ','T')<end]
        item['gpu_monitor']={'samples':len(gpu_rows),'pstates':dict(Counter(r['pstate'] for r in gpu_rows))}
        for field in ['clocks.current.graphics [MHz]','clocks.current.memory [MHz]','utilization.gpu [%]']:
            if gpu_rows:
                item['gpu_monitor'][field]=stats([float(r[field].split()[0]) for r in gpu_rows])
        for field in ['step_wall_ms','tick_wall_ms','snapshot_refresh_ms','physics_callback_before_csv_ms',
                      'prepare_ms','coupling_ms','lbm_ms','rapier_ms','readback_wait_ms','cell_exchange_ms',
                      'submissions','dispatches','readback_calls','readback_bytes']:
            item[field] = stats([float(r[field]) for r in selected])
        trajectories[name] = data
        if name != 'production':
            gd = {int(r['physics_frame']):r for r in rows(root/f'{name}.gd.csv')}
            wall = []
            for interval in intervals:
                frame = int(interval['physics_frame']); r=gd[frame]
                assert int(r['render_pre_frame'])==int(r['render_post_frame'])==frame-1, (name,frame,r)
                # Draw ends immediately before this process callback, after physics's deferred flush.
                assert 0 <= float(r['draw_end_ago_ms']) < float(r['draw_start_ago_ms']) < float(interval['render_interval_ms']), (name,frame,r)
                parts = {k:float(r[k]) for k in ['physics_callback_ms','draw_script_ms','process_callback_prev_ms','pre_render_prev_ms','render_pipeline_prev_ms']}
                parts['engine_unassigned_ms'] = float(interval['render_interval_ms'])-sum(parts.values())
                if name=='phases':
                    parts['after_physics_to_process_ms']=float(r['after_physics_to_process_ms'])
                    parts['after_physics_excluding_world_draw_ms']=float(r['after_physics_to_process_ms'])-float(r['draw_script_ms'])
                    parts['after_render_to_physics_ms']=float(r['after_render_to_physics_ms'])
                    parts['closure_error_ms']=parts['engine_unassigned_ms']-parts['after_physics_excluding_world_draw_ms']-parts['after_render_to_physics_ms']
                    assert abs(parts['closure_error_ms'])<.003,(name,frame,parts)
                wall.append({'physics_frame':frame,'interval_ms':float(interval['render_interval_ms']),**parts,
                             **{k:float(r[k]) for k in ['renderer_setup_cpu_prev_ms','viewport_cpu_cached_ms','viewport_gpu_cached_ms','draw_end_ago_ms']}})
            item['wall'] = {k:stats([r[k] for r in wall]) for k in wall[0] if k!='physics_frame'}
            item['outliers'] = sorted(wall,key=lambda r:r['interval_ms'],reverse=True)[:10]
            logs=log_frames(root/f'{name}.log'); samples={}
            for frame in range(120,3720):
                events=logs[frame]
                for event,keys in [('STEP',['total_ms','prepare_ms','coupling_ms','lbm_ms','rapier_ms','upload_ms','encoding_ms','submit_ms','wait_ms','decode_ms']),
                                   ('MEAN',['reduce_ms','correction_submit_ms']),('CACHE',['read_ms','host_ms']),
                                   ('ADVECTION',['ms']),('SNAPSHOT',['sim_ms','before_return_ms'])]:
                    for key in keys:
                        samples.setdefault(event+'.'+key,[]).append(sum(float(v[key]) for v in events.get(event,[])))
                reads=events['READBACK']; expected=int(data[frame]['readback_calls'])
                assert expected in [3,4] and len(reads)==expected, (name,frame,len(reads),expected)
                for kind,count in [('raw',1),('field',expected-1)]:
                    group=[r for r in reads if r['kind']==kind]; assert len(group)==count
                    for key in ['wait_ms','decode_ms']:
                        samples.setdefault('READBACK.'+kind+'.'+key,[]).append(sum(float(r[key]) for r in group))
                cached=events['CACHE_READ']; assert len(cached)==2
                normal_reads=[cached[0],next(r for r in reads if r['kind']=='raw'),cached[1]]
                for label,read in zip(['intermediate_field','mean_density_raw','final_field'],normal_reads):
                    for key in ['wait_ms','decode_ms']:
                        samples.setdefault('READBACK.'+label+'.'+key,[]).append(float(read[key]))
                if name=='gpu':
                    qs=events['GPU']; assert len(qs)==expected
                    assert all(float(q['kernels_ms'])<=float(q['span_ms'])+.00001 for q in qs)
                    for key in ['passes','kernels_ms','span_ms']:
                        samples.setdefault('GPU.'+key,[]).append(sum(float(q[key]) for q in qs))
                    normal_queries=[qs[next(i for i,r in enumerate(reads) if r is read)] for read in normal_reads]
                    for label,q in zip(['intermediate_field','mean_density_raw','final_field'],normal_queries):
                        for key in ['passes','kernels_ms','span_ms']:
                            samples.setdefault('GPU.'+label+'.'+key,[]).append(float(q[key]))
            item['diagnostic']={k:stats(v) for k,v in samples.items()}
        output[name]=item
    # Timer changes must not silently change simulation work/trajectory.
    comparison={}
    fields=['robot_x','robot_y','density_min','density_max','density_sum','detailed_cells','abstract_cells','region_switches']
    for name,data in trajectories.items():
        comparison[name]={f:max(abs(float(a[f])-float(b[f])) for a,b in zip(trajectories['production'],data)) for f in fields}
    output['trajectory_max_abs_difference_vs_production']=comparison
    (root/'summary.json').write_text(json.dumps(output,indent=2)+'\n')
    for name in names:
        r=output[name]; print(name, 'interval',r['interval_ms'])
        if 'wall' in r:
            print(' wall', {k:round(v['mean'],6) for k,v in r['wall'].items()})
            print(' diagnostic', {k:round(v['mean'],6) for k,v in r['diagnostic'].items()})

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('root',type=Path);args=p.parse_args();analyze(args.root)
