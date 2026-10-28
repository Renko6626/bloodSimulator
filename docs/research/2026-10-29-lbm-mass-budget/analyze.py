#!/usr/bin/env python3
"""Analyze captured populations' mass ledger, without running fluid or assuming a cause."""
import csv,json,math,re,struct
from pathlib import Path
root=Path(__file__).resolve().parent
hardware=root/'hardware-rtx3060-20261029/results'
read=lambda name:list(csv.DictReader((hardware/'budget'/name).open()))
steps=read('steps.csv');openings=read('openings.csv');frames=read('frames.csv');control=read('control.csv')
log=(hardware/'run.log').read_text()
assert (hardware/'exit-code.txt').read_text().strip()=='0' and log.rstrip().endswith('COMPLETE')
assert len(steps)==7000 and len(openings)==21000 and len(frames)==500 and len(control)==3
assert [int(r['step']) for r in steps]==list(range(1,7001))
assert [int(r['frame']) for r in frames]==list(range(500))
assert all(r['bitwise_equal']=='true' and int(r['differing_populations'])==0 for r in control)
f=lambda row,key:float(row[key])
sumcol=lambda rows,key:math.fsum(f(r,key) for r in rows)
maxcol=lambda rows,key:max(abs(f(r,key)) for r in rows)
# Rust logs dt with shortest f32 formatting; recover that exact f32 before f64 integration.
h,dt,D=.25,struct.unpack('<f',struct.pack('<f',float(re.search(r' dt=([^ ]+)',log)[1])))[0],1.
initial_mass=float(re.search(r'initial_mass=(\S+)',log)[1]);time=len(steps)*dt
last=frames[-1]
terms={k:f(last,k) for k in ['mass_change','mass_rel','cum_bulk','cum_reconstruct','cum_incoming','cum_known_out','cum_tangent','cum_link_net_in','cum_chain','cum_compatibility','cum_jn2','cum_macro_trapezoid','cum_budget_residual']}
ledger={key:value for key,value in terms.items() if key not in ['mass_rel']}
physical={k:v*h*h*D for k,v in ledger.items()}
per_opening=[]
for o in range(3):
 rows=[r for r in openings if int(r['opening'])==o]
 q2=sumcol(rows,'q2');j2=sumcol(rows,'j2')
 per_opening.append(dict(opening=o,mean_volume_out=q2/len(rows),mean_rho_mass_out=j2/len(rows),flux_weighted_density=j2/q2,volume_integral_out=q2*dt,mass_integral_out=j2*dt,delta_incoming=sumcol(rows,'delta_incoming'),delta_known_out=sumcol(rows,'delta_known_out'),delta_tangent=sumcol(rows,'delta_tangent'),link_net_in=sumcol(rows,'link_net_in'),jn2=sumcol(rows,'jn2'),b1_minus_c1=sumcol(rows,'b1_minus_c1'),compatibility=sumcol(rows,'compatibility'),q_error_max=max(abs(f(r,'q2')-f(r,'target_q')) for r in rows)))
by_step={}
for r in openings:by_step.setdefault(int(r['step']),[]).append(r)
assert all(len(rows)==3 for rows in by_step.values())
cycles=[]
for start in range(0,500,50):
 batch=frames[start:start+50];cycles.append(dict(first_frame=start,mean_mass_rel=math.fsum(f(r,'mass_rel') for r in batch)/50))
q_out=sum(o['mean_volume_out'] for o in per_opening[1:]);j_out=sum(o['mean_rho_mass_out'] for o in per_opening[1:]);rho_out=j_out/q_out
summary=dict(case='fork_a Fine',adapter='NVIDIA GeForce RTX 3060',software=False,completed_frames=len(frames),lattice_steps=len(steps),simulated_seconds=time,h=h,dt=dt,D=D,initial_mass_lattice_sum=initial_mass,ledger_lattice=terms,ledger_h2D=physical,mean_net_volume_out=sum(o['mean_volume_out'] for o in per_opening),mean_net_rho_mass_out=sum(o['mean_rho_mass_out'] for o in per_opening),max_net_volume_out=max(abs(sum(f(r,'q2') for r in rows)) for rows in by_step.values()),macro_post_integrated_mass_in=-sum(o['mass_integral_out'] for o in per_opening),combined_outlet_flux_weighted_density=rho_out,inlet_minus_combined_outlet_flux_weighted_density=per_opening[0]['flux_weighted_density']-rho_out,per_opening=per_opening,closure=dict(global_step=maxcol(steps,'global_step_residual'),reconstruct_partition=maxcol(steps,'reconstruct_partition_residual'),link=maxcol(steps,'link_residual'),chain_identity=maxcol(steps,'chain_identity_residual'),cumulative_budget=maxcol(steps,'cum_budget_residual'),interstep_mass_continuity=max(abs(f(b,'m0_shift')-f(a,'m2_shift')) for a,b in zip(steps,steps[1:])),placeholder_max_abs=maxcol(steps,'placeholder_max_abs'),placeholder_mismatches=sum(int(r['placeholder_mismatches']) for r in steps)),rho_min=min(f(r,'rho_min') for r in frames),rho_max=max(f(r,'rho_max') for r in frames),max_lattice=max(f(r,'max_lattice') for r in frames),max_q_error=max(f(r,'q_error') for r in frames),prefix_control_bitwise_equal=True,cycles=cycles)
(root/'results/hardware-analysis.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
