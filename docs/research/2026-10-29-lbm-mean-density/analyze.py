#!/usr/bin/env python3
import csv,json,math,re,struct
from pathlib import Path
root=Path(__file__).resolve().parent
hw=root/'hardware-rtx3060-20261029/results'
raw=hw/'paired'
read=lambda name:list(csv.DictReader((raw/name).open()))
baseline=read('baseline.csv');controlled=read('controlled.csv');corrections=read('corrections.csv');comparison=read('comparison.csv');forced=read('force-check.csv')
log=(hw/'run.log').read_text()
assert (hw/'exit-code.txt').read_text().strip()=='0' and log.rstrip().endswith('COMPLETE guardrail_breaches=0')
assert len(baseline)==len(controlled)==len(corrections)==1000 and len(comparison)==2000 and len(forced)==1
for rows in [baseline,controlled,corrections]:assert [int(r['frame']) for r in rows]==list(range(1000))
assert all(r['passed']=='true' for r in corrections) and forced[0]['passed']=='true'
f=lambda r,k:float(r[k])
maxkey=lambda rows,k:dict(value=max(f(r,k) for r in rows),frame=int(max(rows,key=lambda r:f(r,k))['frame']))
series=lambda rows:dict(final={k:float(v) for k,v in rows[-1].items()},mean_residual_max=max(abs(f(r,'mean_rho')-1) for r in rows),rho_min=min(f(r,'rho_min') for r in rows),rho_max=max(f(r,'rho_max') for r in rows),max_lattice=maxkey(rows,'max_lattice'),max_velocity_error_world=maxkey(rows,'velocity_error_world'),max_volume_flux_error=maxkey(rows,'volume_flux_error'))
regions={}
keys=['velocity_rel_l2','velocity_abs_rms_lattice','velocity_abs_max_lattice','demeaned_rho_rel_l2','demeaned_rho_abs_rms','demeaned_rho_abs_max','neqstress_rel_l2','neqstress_abs_rms','neqstress_abs_max']
for region in ['global','central']:
 rows=[r for r in comparison if r['region']==region];assert len(rows)==1000 and [int(r['frame']) for r in rows]==list(range(1000))
 regions[region]=dict(peaks={k:maxkey(rows,k) for k in keys},final={k:f(rows[-1],k) for k in keys},guardrail_breach_frames=[int(r['frame']) for r in rows if r['guardrail_passed']!='true'])
removed=[f(r,'actual_removed') for r in corrections]
summary=dict(case='fork_a Fine',frames_per_window=1000,lattice_steps_per_window=14000,reference_density=1.,adapter='NVIDIA GeForce RTX 3060',software=False,baseline=series(baseline),controlled=series(controlled),instantaneous={k:maxkey(corrections,k) for k in ['mean_residual','density_shift_residual','velocity_change','neqstress_component_change']},max_abs_removal_error=max(abs(f(r,'removal_error')) for r in corrections),maximum_abs_delta_f32=max(abs(f(r,'delta_f32')) for r in corrections),positive_mass_removed=math.fsum(v for v in removed if v>0),positive_mass_added=-math.fsum(v for v in removed if v<0),net_mass_removed=math.fsum(removed),net_mass_removed_h2D=math.fsum(removed)*.25**2,cumulative_budget_residual_max=max(abs(f(r,'budget_residual')) for r in controlled),regions=regions,force_check={k:float(v) if k!='passed' else v=='true' for k,v in forced[0].items()},predeclared=dict(instantaneous_absolute=5e-7,paired_velocity_rel_l2=.05,paired_demeaned_rho_rel_l2=.05,paired_neqstress_rel_l2=.10))
(root/'results/hardware-analysis.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
