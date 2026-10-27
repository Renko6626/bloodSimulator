#!/usr/bin/env python3
"""Aggregate cloud profile and resolve sampled PCs against archived exact ELF files."""
import argparse,collections,csv,hashlib,json,math,statistics,struct,subprocess
from pathlib import Path
STAGES={1:'advection',2:'drive',3:'diffusion_u',4:'diffusion_v',5:'pressure_rhs',6:'pressure_solve',7:'pressure_correct',8:'coupling',9:'rapier',10:'diagnostics'}
def read(path):
    with path.open() as f:return list(csv.DictReader(f))
def stats(xs):
    return {'mean':statistics.mean(xs),'p95':sorted(xs)[math.ceil(.95*len(xs))-1],'max':max(xs)}
def symbolicate(out,path):
    info=json.loads(path.with_suffix(path.suffix+'.json').read_text())
    assert info['timer_failures']==0 and info['records']==info['stored']
    records=list(struct.iter_unpack('<QII',path.read_bytes()));assert len(records)==info['records']
    maps=[]
    for line in path.with_suffix(path.suffix+'.maps').read_text().splitlines():
        fields=line.split(maxsplit=5)
        if len(fields)==6 and 'x' in fields[1]:
            low,high=(int(x,16) for x in fields[0].split('-'));maps.append((low,high,int(fields[2],16),fields[5]))
    module_map=json.loads((out/'module-map.json').read_text())
    # ELF PT_LOAD virtual addresses need not equal file offsets (lld uses gaps).
    segments={}
    for module,item in module_map.items():
        data=(out/item['file']).read_bytes();assert data[:6]==b'\x7fELF\x02\x01'
        phoff=struct.unpack_from('<Q',data,32)[0];entry_size,count=struct.unpack_from('<HH',data,54)
        segments[module]=[]
        for i in range(count):
            kind,flags,offset,vaddr,_,filesz,_,_=struct.unpack_from('<IIQQQQQQ',data,phoff+i*entry_size)
            if kind==1 and flags&1:segments[module].append((offset,vaddr,filesz))
    addr_map={};per_module=collections.defaultdict(set)
    for pc in {x[0] for x in records}:
        for low,high,offset,module in maps:
            if low<=pc<high:
                filepos=pc-low+offset
                for fileoff,vaddr,size in segments.get(module,[]):
                    if fileoff<=filepos<fileoff+size:
                        address=vaddr+filepos-fileoff
                        addr_map[pc]=(module,address);per_module[module].add(address);break
                break
    symbols={}
    for module,addresses in per_module.items():
        if module not in module_map:continue
        addresses=sorted(addresses);binary=out/module_map[module]['file']
        proc=subprocess.run(['addr2line','-f','-C','-e',str(binary)],input=''.join(f'{x:x}\n' for x in addresses),text=True,capture_output=True,check=True)
        lines=proc.stdout.splitlines();assert len(lines)==2*len(addresses),(module,len(lines),len(addresses))
        for i,addr in enumerate(addresses):symbols[module,addr]=(lines[2*i],lines[2*i+1])
    counts=collections.Counter();totals=collections.Counter();threads=collections.Counter();unknown=0
    for pc,tid,stage in records:
        module,addr=addr_map.get(pc,('unmapped',pc));fn,source=symbols.get((module,addr),('??','??:0'))
        if fn=='??':unknown+=1
        counts[stage,module,fn,source]+=1;totals[stage]+=1;threads[stage,tid]+=1
    rows=[]
    for (stage,module,fn,source),n in counts.most_common():
        rows.append({'stage':STAGES.get(stage,str(stage)),'module':module,'function':fn,'source':source,'samples':n,'stage_percent':100*n/totals[stage],'all_percent':100*n/len(records)})
    result={'metadata':info,'unknown_symbols':unknown,'stage_samples':{STAGES[k]:v for k,v in totals.items()},'thread_samples':[{'stage':STAGES[s],'tid':tid,'samples':n,'main':tid==info['main_tid']} for (s,tid),n in threads.most_common()],'hotspots':rows}
    path.with_suffix('.hotspots.json').write_text(json.dumps(result,indent=2)+'\n')
    with path.with_suffix('.hotspots.csv').open('w') as f:
        writer=csv.DictWriter(f,fieldnames=list(rows[0]));writer.writeheader();writer.writerows(rows)
    return result

def main():
    p=argparse.ArgumentParser();p.add_argument('results',type=Path);args=p.parse_args();out=args.results.resolve()
    manifest=json.loads((out/'manifest.json').read_text());assert manifest['status']=='complete'
    checks=json.loads((out/'checksums.json').read_text())
    for name,expected in checks.items():assert hashlib.sha256((out/name).read_bytes()).hexdigest()==expected,name
    runs=[];groups=collections.defaultdict(list);details=[]
    for path in sorted(out.glob('r*-*.json')):
        data=json.loads(path.read_text());rows=read(path.with_suffix('.csv'));assert len(rows)==data['frames']
        for row in rows:assert all(math.isfinite(float(v)) for v in row.values())
        variant=path.stem.split('-',2)[2];n=data['nx']
        timing={field:stats([float(r[field]) for r in rows]) for field in ['advection_ms','diffusion_ms','pressure_ms','coupling_ms','rapier_ms','diagnostics_ms','fluid_coupling_ms','whole_ms']}
        run={'name':path.stem,'nx':n,'variant':variant,'timing':timing,'field_fingerprint':data.get('field_fingerprint'),'final_mean_u':data['final_mean_u'],'final_divergence_rms':data['final_divergence_rms']}
        runs.append(run);groups[n,variant].extend(rows)
        sp=path.with_suffix('.stages.csv')
        if sp.exists():
            by_stage=collections.defaultdict(lambda:collections.defaultdict(lambda:collections.defaultdict(float)))
            for r in read(sp):
                for field in ['wall_ms','process_cpu_ms','main_cpu_ms']:by_stage[r['stage']][r['frame']][field]+=float(r[field])
            for stage,frames in by_stage.items():
                assert len(frames)==data['frames']
                details.append({'name':path.stem,'nx':n,'variant':variant,'stage':stage,**{field:stats([r[field] for r in frames.values()]) for field in ['wall_ms','process_cpu_ms','main_cpu_ms']}})
    assert len(runs)==20 and len(groups)==10
    assert all(len(rows)==240 for rows in groups.values())
    assert len(list(out.glob('*.samples')))==2
    summary=[]
    for (n,variant),rows in sorted(groups.items()):
        summary.append({'nx':n,'variant':variant,'samples':len(rows),'timing':{field:stats([float(r[field]) for r in rows]) for field in ['advection_ms','diffusion_ms','pressure_ms','coupling_ms','rapier_ms','diagnostics_ms','fluid_coupling_ms','whole_ms']}})
    validation={}
    physical_fields=['t','divergence_rms','energy','max_component','mean_u','max_area_rel','max_node_speed','cell_mean_vx','exchange_mismatch']
    for n in [192,256]:
        configs=[x for x in runs if x['nx']==n]
        fingerprints={x['field_fingerprint'] for x in configs if x['field_fingerprint']}
        assert len(fingerprints)==1,(n,fingerprints)
        ref=read(out/f'r1-{n}-baseline.csv')
        max_delta={k:0.0 for k in physical_fields}
        names=[x['name'] for x in configs]+[f'sample-{n}-t4']
        for name in names:
            observed=read(out/(name+'.csv'))
            for a,b in zip(ref,observed):
                for k in physical_fields:max_delta[k]=max(max_delta[k],abs(float(a[k])-float(b[k])))
        assert all(x==0.0 for x in max_delta.values()),(n,max_delta)
        validation[str(n)]={'profile_binary_final_field_fingerprint':next(iter(fingerprints)),'baseline_and_variants_and_sample_first120_max_absolute_delta_in_serialized_observables':max_delta}
    samples={p.stem:symbolicate(out,p) for p in sorted(out.glob('*.samples'))}
    result={'validation':validation,'runs':runs,'combined':summary,'detail':details,'sample_runs':{k:{a:v[a] for a in ['metadata','unknown_symbols','stage_samples','thread_samples']} for k,v in samples.items()}}
    (out/'profile-summary.json').write_text(json.dumps(result,indent=2)+'\n')
    lines=['# Vast NS profile 原始汇总','','| 网格 | 观测组 | 完整 mean / p95 ms | 流体+耦合 mean / p95 ms |','| --- | --- | ---: | ---: |']
    for row in summary:
        a=row['timing']['whole_ms'];b=row['timing']['fluid_coupling_ms'];lines.append(f"| {row['nx']}² | {row['variant']} | {a['mean']:.3f} / {a['p95']:.3f} | {b['mean']:.3f} / {b['p95']:.3f} |")
    lines+=['','off=观测关闭；detail=分段 CPU／墙钟计时；数字为 Rayon 工作线程数。baseline 为首批原始二进制。每组两轮，共 240 测量帧。','', '| 网格 | 组 | 阶段 | mean wall ms/frame | mean process CPU ms/frame | mean main CPU ms/frame |','| --- | --- | --- | ---: | ---: | ---: |']
    detail_groups=collections.defaultdict(list)
    for row in details:detail_groups[row['nx'],row['variant'],row['stage']].append(row)
    for (n,variant,stage),rows in sorted(detail_groups.items()):
        fields=[statistics.mean(x[f]['mean'] for x in rows) for f in ['wall_ms','process_cpu_ms','main_cpu_ms']]
        lines.append(f"| {n}² | {variant} | {stage} | {fields[0]:.3f} | {fields[1]:.3f} | {fields[2]:.3f} |")
    for name,sample in samples.items():
        lines+=['',f'## {name}',json.dumps(sample['metadata']), '']
        for stage in ['advection','diffusion_u','diffusion_v','pressure_solve','rapier']:
            lines.append(f'### {stage}')
            # Aggregate source locations to leaf function for concise top list.
            functions=collections.Counter()
            for row in sample['hotspots']:
                if row['stage']==stage:functions[row['function']]+=row['samples']
            for fn,count in functions.most_common(8):lines.append(f"- {count} ({100*count/sample['stage_samples'][stage]:.1f}%): {fn}")
    (out/'profile-summary.md').write_text('\n'.join(lines)+'\n')
    print('\n'.join(lines[:15]))
if __name__=='__main__':main()
