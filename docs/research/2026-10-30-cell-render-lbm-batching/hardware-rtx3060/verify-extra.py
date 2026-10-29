from pathlib import Path
import hashlib,json
root=Path('/workspace/blood-cell-batch-rtx');old=Path('/workspace/blood-game-e2e/current')
digest=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
result={}
for name,world,hud in [('hud-only',old,root/'candidate'),('mesh-only',root/'candidate',old)]:
 project=root/name
 assert digest(project/'godot/world.gd')==digest(world/'godot/world.gd')
 assert digest(project/'godot/world_hud.gd')==digest(hud/'godot/world_hud.gd')
 assert digest(project/'target/debug/libgodot_bridge.so')=='69deadd806c1abc93f3c84deb3948e0565c89ca350cc50c2ce2de6bec3f32ec6'
 result[name]={p:digest(project/p) for p in ['godot/world.gd','godot/world_hud.gd','target/debug/libgodot_bridge.so']}
assert digest(root/'mesh-only/godot/world_cell_mesh.gd')==digest(root/'candidate/godot/world_cell_mesh.gd')
(root/'extra-source-verified.json').write_text(json.dumps(result,indent=2)+'\n')
