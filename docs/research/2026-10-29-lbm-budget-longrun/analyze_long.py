#!/usr/bin/env python3
import csv,hashlib,json,math,re,sys
from pathlib import Path
root=Path(__file__).resolve().parent
raw=Path(sys.argv[1]) if len(sys.argv)>1 else root/'hardware-rtx3060-20261029/results/long'
logpath=Path(sys.argv[2]) if len(sys.argv)>2 else root/'hardware-rtx3060-20261029/results/long.log'
read=lambda name:list(csv.DictReader((raw/name).open()))
b=read('baseline.csv');c=read('controlled.csv');cor=read('corrections.csv');cmp=read('comparison.csv');force=read('force-check.csv')
n=len(b);assert n>0 and len(c)==n and len(cmp)==2*n and len(cor) in [n,n+1]
for rows in [b,c,cor]:assert [int(r['frame']) for r in rows]==list(range(len(rows)))
f=lambda r,k:float(r[k])
maxkey=lambda rows,k:dict(value=max(f(r,k) for r in rows),frame=int(max(rows,key=lambda r:f(r,k))['frame']))
series=lambda rows:dict(final={k:float(v) for k,v in rows[-1].items()},rho_min=min(f(r,'rho_min') for r in rows),rho_max=max(f(r,'rho_max') for r in rows),max_lattice=maxkey(rows,'max_lattice'),max_mean_residual=max(abs(f(r,'mean_rho')-1) for r in rows),max_velocity_error_world=maxkey(rows,'velocity_error_world'),max_volume_flux_error=maxkey(rows,'volume_flux_error'))
keys=['velocity_rel_l2','velocity_abs_rms_lattice','velocity_abs_max_lattice','demeaned_rho_rel_l2','demeaned_rho_abs_rms','demeaned_rho_abs_max','neqstress_rel_l2','neqstress_abs_rms','neqstress_abs_max']
regions={}
for region in ['global','central']:
 rows=[r for r in cmp if r['region']==region];assert len(rows)==n and [int(r['frame']) for r in rows]==list(range(n))
 regions[region]=dict(peaks={k:maxkey(rows,k) for k in keys},final={k:f(rows[-1],k) for k in keys},guardrail_breach_frames=[int(r['frame']) for r in rows if r['guardrail_passed']!='true'])
blocks=[]
for start in range(0,n,1000):
 stop=min(n,start+1000);bb=b[start:stop];cc=c[start:stop]
 blocks.append(dict(first_frame=start,last_frame=stop-1,baseline_mean_average=math.fsum(f(r,'mean_rho') for r in bb)/len(bb),baseline_mean_last=f(bb[-1],'mean_rho'),controlled_mean_max_residual=max(abs(f(r,'mean_rho')-1) for r in cc),cumulative_removed_at_end=f(cc[-1],'cum_removed_mass'),field_peaks={region:{k:max(f(r,k) for r in cmp if r['region']==region and start<=int(r['frame'])<stop) for k in keys} for region in ['global','central']}))
cycles=[dict(first_frame=i,frames=len(b[i:i+50]),baseline_mean_rho=math.fsum(f(r,'mean_rho') for r in b[i:i+50])/len(b[i:i+50])) for i in range(0,n,50)]
prefix={}
old=root.parent/'2026-10-29-lbm-mean-density/hardware-rtx3060-20261029/results/paired'
for name in ['baseline.csv','controlled.csv','corrections.csv','comparison.csv','force-check.csv']:
 before=(old/name).read_text().splitlines();now=(raw/name).read_text().splitlines();prefix[name]=now[:len(before)]==before
log=logpath.read_text();done=re.search(r'COMPLETE guardrail_breaches=(\d+)\s*$',log)
removed=[f(r,'actual_removed') for r in cor if r['passed']=='true']
summary=dict(frames_requested=10000,completed_frames_per_window=n,complete=bool(done) and n==10000,reported_guardrail_breach_frames=int(done[1]) if done else None,baseline=series(b),controlled=series(c),instantaneous={k:maxkey(cor,k) for k in ['mean_residual','density_shift_residual','velocity_change','neqstress_component_change']},failed_correction_frames=[int(r['frame']) for r in cor if r['passed']!='true'],maximum_abs_delta=max(abs(f(r,'delta_f32')) for r in cor),max_abs_removal_error=max(abs(f(r,'removal_error')) for r in cor),positive_mass_removed=math.fsum(v for v in removed if v>0),positive_mass_added=-math.fsum(v for v in removed if v<0),net_mass_removed=math.fsum(removed),net_mass_removed_h2D=math.fsum(removed)*.25**2,budget_residual_max=max(abs(f(r,'budget_residual')) for r in c),regions=regions,blocks_1000=blocks,cycles_50=cycles,previous_1000_csv_prefixes_match=prefix,force_check={k:float(v) if k!='passed' else v=='true' for k,v in force[0].items()},input_sha256={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in raw.glob('*.csv')})
(root/'results/longrun-analysis.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
