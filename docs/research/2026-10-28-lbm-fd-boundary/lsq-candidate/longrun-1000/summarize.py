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
    assert len(rows) == 1000, 'Unexpected completed frame count'
cycles = []
for start in range(0,len(rows),50):
    batch = rows[start:start+50]
    cycles.append(dict(first_frame=start, frames=len(batch), mean_mass_rel=sum(r['mass_rel'] for r in batch)/len(batch), rho_min=min(r['rho_min'] for r in batch), rho_max=max(r['rho_max'] for r in batch), max_lattice=max(r['max_lattice'] for r in batch)))
old = (p.parent / 'results/fork_a-fine.log').read_text().splitlines()
old = [s for s in old if s.startswith('frame=')]
new = [s for s in text.splitlines() if s.startswith('frame=')]
summary = dict(complete=complete, completed_frames=len(rows), lattice_steps_completed=len(rows)*14, simulated_seconds_completed=len(rows)/60, rho_min=min(r['rho_min'] for r in rows), rho_max=max(r['rho_max'] for r in rows), max_lattice=max(r['max_lattice'] for r in rows), max_velocity_error=max(r['velocity_error'] for r in rows), max_flux_error=max(r['flux_error'] for r in rows), mass_rel_min=min(r['mass_rel'] for r in rows), mass_rel_max=max(r['mass_rel'] for r in rows), final=rows[-1], original_160_frame_lines_match=(new[:len(old)] == old), cycles=cycles)
(p/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps(summary,indent=2))
