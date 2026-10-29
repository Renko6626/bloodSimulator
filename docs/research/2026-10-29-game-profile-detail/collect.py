#!/usr/bin/env python3
"""Run on the experiment host after run.sh; collect only this round's evidence."""
import hashlib, json, shutil, tarfile
from pathlib import Path
root=Path('/workspace/blood-game-profile-detail')
assert (root/'results/runner.complete').read_text().strip()=='COMPLETE'
manifest=json.loads((root/'manifest.json').read_text())
digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
for name in ['production','cpu','render','gpu','phases']:
    assert (root/f'results/{name}.exit.txt').read_text().strip()=='0'
    assert (root/f'results/{name}.csv.status.txt').read_text().strip()=='COMPLETE'
assert digest(root/'instrumented/target/debug/libgodot_bridge.so')==manifest['instrumented_library_sha256']
assert digest(Path('/workspace/blood-game-e2e/current/target/debug/libgodot_bridge.so'))==manifest['production_library_sha256']
assert digest(Path('/workspace/blood-game-e2e/godot-bin'))==manifest['godot_sha256']
assert digest(root/'phases/godot/world.gd')==manifest['phases_world_sha256']
for path,h in manifest['production_godot_sha256'].items():
    assert digest(Path('/workspace/blood-game-e2e/current')/path)==h,path
out=root/'evidence';out.mkdir(exist_ok=True)
for path,h in manifest['source_sha256'].items():
    p=root/'instrumented'/path; assert digest(p)==h,path
    target=out/'diagnostic-source'/path;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,target)
for name in ['results','results-preflight-v1','results-preflight-v2','display']:
    shutil.copytree(root/name,out/name,dirs_exist_ok=True)
target=out/'phases-source/godot/world.gd';target.parent.mkdir(parents=True,exist_ok=True)
shutil.copy2(root/'phases/godot/world.gd',target)
for name in ['manifest.json','import.log','run.sh','preflight.sh','runner.log','run-phases.sh','phases-runner.log']:
    shutil.copy2(root/name,out/name)
hashes={str(p.relative_to(out)):digest(p) for p in sorted(out.rglob('*')) if p.is_file() and p.name!='artifact-sha256.json'}
(out/'artifact-sha256.json').write_text(json.dumps(hashes,indent=2)+'\n')
with tarfile.open(root/'evidence.tar.gz','w:gz') as archive:
    archive.add(out,arcname='evidence')
print(json.dumps({'files':len(hashes),'bundle_sha256':digest(root/'evidence.tar.gz')},indent=2))
