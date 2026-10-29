#!/usr/bin/env bash
set -euo pipefail
cd /workspace/blood-cell-batch-rtx
export DISPLAY=:99 XDG_RUNTIME_DIR=/tmp/blood-cell-batch-runtime
export LD_LIBRARY_PATH="/workspace/blood-game-e2e/display/glvnd/usr/lib/x86_64-linux-gnu:/workspace/blood-game-e2e/display/driver:${LD_LIBRARY_PATH:-}"
mkdir -p "$XDG_RUNTIME_DIR" results
chmod 700 "$XDG_RUNTIME_DIR"
glxinfo -B > display/glxinfo.txt
nvidia-smi --query-gpu=name,uuid,driver_version,pci.bus_id --format=csv > display/gpu.csv
cat /sys/fs/cgroup/cpu.max > display/cpu-max.txt
lscpu > display/lscpu.txt
tar -xzf payload.tar.gz
mkdir -p candidate/target/release
ln -s ../debug/libgodot_bridge.so candidate/target/release/libgodot_bridge.so
cp -a /workspace/blood-game-e2e/current/godot/.godot candidate/godot/.godot
cp -a candidate render-only
cp /workspace/blood-game-e2e/current/target/debug/libgodot_bridge.so render-only/target/debug/libgodot_bridge.so
cp -a /workspace/blood-game-e2e/current lbm-only
cp candidate/target/debug/libgodot_bridge.so lbm-only/target/debug/libgodot_bridge.so
for project in candidate render-only lbm-only; do
 /workspace/blood-game-e2e/godot-bin --headless --path "$project/godot" --editor --import --quit > "$project.import.log" 2>&1
 printf '0\n' > "$project.import.exit.txt"
done
/workspace/blood-game-e2e/godot-bin --path candidate/godot --audio-driver Dummy --script res://tests/world_cell_mesh_test.gd > mesh-test.log 2>&1
python3 -c 'assert "CELL_MESH_TEST PASS" in open("mesh-test.log").read()'
printf 'SETUP_COMPLETE\n'
