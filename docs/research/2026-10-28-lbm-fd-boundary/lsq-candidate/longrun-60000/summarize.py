import json
from pathlib import Path
import re
p = Path(__file__).resolve().parent
text = (p / 'run.log').read_text()
pattern = re.compile(r'frame=(\d+) rho=\[([^,]+),([^\]]+)\] mass_rel=(\S+) max_lattice=(\S+) velocity_error=(\S+) flux_error=(\S+)')
rows = [dict(zip(('frame','rho_min','rho_max','mass_rel','max_lattice','velocity_error','flux_error'), (int(m[0]), *map(float,m[1:])))) for m in pattern.findall(text)]
assert rows, 'No completed-frame diagnostics'
complete = text.rstrip().endswith('COMPLETE') and (p / 'exit-code.txt').read_text().strip() == '0'
assert [r['frame'] for r in rows] == list(range(len(rows))), 'Nonconsecutive frames'
if complete:
    assert len(rows) == 60000, 'Unexpected completed frame count'
cycles = []
for start in range(0,len(rows),50):
    batch = rows[start:start+50]
    cycles.append(dict(first_frame=start, frames=len(batch), mean_mass_rel=sum(r['mass_rel'] for r in batch)/len(batch), rho_min=min(r['rho_min'] for r in batch), rho_max=max(r['rho_max'] for r in batch), max_lattice=max(r['max_lattice'] for r in batch)))
old = (p.parent / 'results/fork_a-fine.log').read_text().splitlines()
old = [s for s in old if s.startswith('frame=')]
new = [s for s in text.splitlines() if s.startswith('frame=')]
old1000 = (p.parent / 'longrun-1000/run.log').read_text().splitlines()
old1000 = [line for line in old1000 if line.startswith('frame=')]
full_cycles = [c for c in cycles if c['frames'] == 50]
blocks = []
for start in range(0,len(rows),10000):
    batch = [c for c in full_cycles if start <= c['first_frame'] < start+10000]
    if len(batch) < 2:
        continue
    x = [c['first_frame']+24.5 for c in batch]
    y = [c['mean_mass_rel'] for c in batch]
    mx,my=sum(x)/len(x),sum(y)/len(y)
    slope=sum((a-mx)*(b-my) for a,b in zip(x,y))/sum((a-mx)**2 for a in x)
    residual=sum((b-my-slope*(a-mx))**2 for a,b in zip(x,y))
    variance=sum((b-my)**2 for b in y)
    blocks.append(dict(first_frame=start, full_cycles=len(batch), first_cycle_mean_mass_rel=y[0], last_cycle_mean_mass_rel=y[-1], mean_mass_rel=sum(y)/len(y), linear_slope_mass_rel_per_frame=slope, linear_slope_percentage_points_per_1000_frames=slope*100*1000, r_squared=1-residual/variance if variance else None, positive_cycle_increments=sum(b>a for a,b in zip(y,y[1:]))))
cycle_increments=[b['mean_mass_rel']-a['mean_mass_rel'] for a,b in zip(full_cycles,full_cycles[1:])]
summary = dict(complete=complete, completed_frames=len(rows), lattice_steps_completed=len(rows)*14, simulated_seconds_completed=len(rows)/60, rho_min=min(r['rho_min'] for r in rows), rho_max=max(r['rho_max'] for r in rows), max_lattice=max(r['max_lattice'] for r in rows), max_velocity_error=max(r['velocity_error'] for r in rows), max_flux_error=max(r['flux_error'] for r in rows), mass_rel_min=min(r['mass_rel'] for r in rows), mass_rel_max=max(r['mass_rel'] for r in rows), final=rows[-1], original_160_frame_lines_match=(new[:len(old)] == old), cycles=cycles, full_cycles=len(full_cycles), positive_full_cycle_increments=sum(d>0 for d in cycle_increments), full_cycle_increment_min=min(cycle_increments) if cycle_increments else None, full_cycle_increment_max=max(cycle_increments) if cycle_increments else None, original_1000_frame_lines_match=(new[:len(old1000)] == old1000), blocks_10000_frames=blocks, abnormal_lines=[line for line in text.splitlines() if line.startswith('BAD') or 'panicked' in line])
(p/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
