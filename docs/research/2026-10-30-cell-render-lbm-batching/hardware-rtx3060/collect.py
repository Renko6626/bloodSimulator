import json,hashlib,shutil,tarfile
from pathlib import Path
root=Path('/workspace/blood-cell-batch-rtx');out=root/'evidence'
assert (root/'results/runner.complete').read_text().strip()=='COMPLETE'
for name in ['baseline-a','candidate-a','render-only','lbm-only','candidate-b','baseline-b','capture','hud-only','mesh-only']:
 assert (root/f'results/{name}.exit.txt').read_text().strip()=='0',name
 assert (root/f'results/{name}.csv.status.txt').read_text().strip()=='COMPLETE',name
assert (root/'results/extra.complete').read_text().strip()=='COMPLETE'
out.mkdir(exist_ok=True)
for name in ['results','display']:shutil.copytree(root/name,out/name,dirs_exist_ok=True)
for name in ['manifest.json','run.sh','setup.sh','capture.sh','verify-source.py','source-verified.txt','runner.log','setup.log','mesh-test.log','candidate.import.log','candidate.import.exit.txt','render-only.import.log','render-only.import.exit.txt','lbm-only.import.log','lbm-only.import.exit.txt','run-extra.sh','verify-extra.py','extra-source-verified.json','extra-runner.log','hud-only.import.log','hud-only.import.exit.txt','mesh-only.import.log','mesh-only.import.exit.txt']:
 shutil.copy2(root/name,out/name)
m=json.loads((root/'manifest.json').read_text());digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert digest(root/'candidate/target/debug/libgodot_bridge.so')==m['candidate_library_sha256']
assert digest(Path('/workspace/blood-game-e2e/current/target/debug/libgodot_bridge.so'))==m['baseline_library_sha256']
for name,h in m['candidate_godot_sha256'].items():
 p=root/'candidate'/name;assert digest(p)==h,name
 target=out/'candidate-source'/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,target)
for name in ['world.gd','world_hud.gd']:
 target=out/'baseline-source/godot'/name;target.parent.mkdir(parents=True,exist_ok=True)
 shutil.copy2(Path('/workspace/blood-game-e2e/current/godot')/name,target)
hashes={str(p.relative_to(out)):digest(p) for p in sorted(out.rglob('*')) if p.is_file() and p.name!='artifact-sha256.json'}
(out/'artifact-sha256.json').write_text(json.dumps(hashes,indent=2)+'\n')
with tarfile.open(root/'evidence.tar.gz','w:gz') as t:t.add(out,arcname='evidence')
print(json.dumps({'files':len(hashes),'bundle_sha256':digest(root/'evidence.tar.gz')},indent=2))
