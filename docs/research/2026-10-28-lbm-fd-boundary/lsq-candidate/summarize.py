#!/usr/bin/env python3
import json
import re
from pathlib import Path
root=Path(__file__).resolve().parent
summary={}
for case in ['diagonal45-fine','diagonal45-coarse','fork_a-coarse','fork_b-coarse','fork_a-fine']:
    text=(root/f'results/{case}.log').read_text()
    rows=[line for line in text.splitlines() if line.startswith('frame=')]
    assert len(rows)==160 and text.rstrip().endswith('COMPLETE')
    vals=lambda key:[float(re.search(rf'{key}=([^ ]+)',r)[1]) for r in rows]
    rho=[tuple(map(float,re.search(r'rho=\[([^,]+),([^]]+)\]',r).groups())) for r in rows]
    stats=json.loads((root/f'results/{case}.geom.stats.json').read_text())
    manufacture=(root/f'results/{case}-manufactured.log').read_text()
    errors=[float(e) for e in re.findall(r'max_absolute_gradient_error=([^ ]+)',manufacture)]
    assert len(errors)==2 and max(errors)<=5e-8
    summary[case]=dict(frames=len(rows),rho_min=min(r[0] for r in rho),rho_max=max(r[1] for r in rho),max_lattice=max(vals('max_lattice')),mass_rel_min=min(vals('mass_rel')),mass_rel_max=max(vals('mass_rel')),mass_rel_final=vals('mass_rel')[-1],max_boundary_velocity_error=max(vals('velocity_error')),max_flux_error=max(vals('flux_error')),manufactured_errors=errors,stencil={k:v for k,v in stats.items() if k!='rows'})
output=json.dumps(summary,indent=2)+'\n'
(root/'results/summary.json').write_text(output)
print(output,end='')
