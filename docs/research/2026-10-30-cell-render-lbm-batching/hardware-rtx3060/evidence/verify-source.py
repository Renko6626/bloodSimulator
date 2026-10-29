import json,hashlib
from pathlib import Path
root=Path('/workspace/blood-cell-batch-rtx');old=Path('/workspace/blood-game-e2e/current')
m=json.loads((root/'manifest.json').read_text());digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
for p,h in m['candidate_godot_sha256'].items():assert digest(root/'candidate'/p)==h,p
assert digest(root/'candidate/target/debug/libgodot_bridge.so')==m['candidate_library_sha256']
assert digest(old/'target/debug/libgodot_bridge.so')==m['baseline_library_sha256']
assert digest(old/'godot/world.gd')=='d7029fbce3f1f66b53bffe328a1b4f0acee9f926dc4ef1595cbb6e19486b914e'
assert digest(old/'godot/world_hud.gd')=='b6e4f00a374be7234623e4253c9c7450f0c5a9c94a36e9c3224339306c32d18b'
for name,script_base,lib in [('render-only',root/'candidate',m['baseline_library_sha256']),('lbm-only',old,m['candidate_library_sha256'])]:
 assert digest(root/name/'target/debug/libgodot_bridge.so')==lib,name
 for p in ['godot/world.gd','godot/world_hud.gd']:
  assert digest(root/name/p)==digest(script_base/p),(name,p)
assert digest(Path('/workspace/blood-game-e2e/godot-bin'))=='32f8d7596c4b41185512b1c49d69f2da3be018fd784a53e349fa92a98a97bcde'
(root/'source-verified.txt').write_text('PASS: source, libraries, crossed controls and Godot binary hashes verified\n')
print((root/'source-verified.txt').read_text())
