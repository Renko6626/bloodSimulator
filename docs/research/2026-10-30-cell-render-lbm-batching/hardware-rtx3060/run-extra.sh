#!/usr/bin/env bash
set -euo pipefail
cd /workspace/blood-cell-batch-rtx
export DISPLAY=:99 XDG_RUNTIME_DIR=/tmp/blood-cell-batch-runtime
export LD_LIBRARY_PATH="/workspace/blood-game-e2e/display/glvnd/usr/lib/x86_64-linux-gnu:/workspace/blood-game-e2e/display/driver:${LD_LIBRARY_PATH:-}"
mkdir -p "$XDG_RUNTIME_DIR" results
chmod 700 "$XDG_RUNTIME_DIR"
GODOT_BIN=/workspace/blood-game-e2e/godot-bin
nvidia-smi --query-gpu=name,uuid,driver_version,pstate,clocks.current.graphics,clocks.current.memory --format=csv > results/extra-gpu-before.csv
nvidia-smi --query-gpu=timestamp,pstate,clocks.current.graphics,clocks.current.memory,utilization.gpu,memory.used,power.draw --format=csv -lms 200 > results/extra-gpu-timeline.csv &
monitor_pid=$!
trap 'kill "$monitor_pid" 2>/dev/null || true' EXIT
run() {
 local name=$1 project=$2 frames=$3
 shift 3
 date -u +%FT%TZ > "results/$name.started.txt"
 set +e
 timeout --kill-after=10 600 env -u BLOOD_PROFILE -u BLOOD_GPU_PROFILE -u BLOOD_GODOT_GPU_PROFILE -u BLOOD_PROFILE_VISUAL "$@" \
   "$GODOT_BIN" --path "$project/godot" --fixed-fps 60 --disable-vsync --audio-driver Dummy -- \
   --local-lbm --lbm-validation --local-cells --lbm-benchmark=move --lbm-benchmark-warmup=120 --lbm-benchmark-frames="$frames" \
   --lbm-benchmark-output="$PWD/results/$name.csv" > "results/$name.log" 2>&1
 local code=$?
 set -e
 printf '%s\n' "$code" > "results/$name.exit.txt"
 date -u +%FT%TZ > "results/$name.finished.txt"
 printf '%s exit=%s\n' "$name" "$code" | tee -a results/runner-status.txt
}
cp -a /workspace/blood-game-e2e/current hud-only
cp candidate/godot/world_hud.gd hud-only/godot/world_hud.gd
cp -a render-only mesh-only
cp /workspace/blood-game-e2e/current/godot/world_hud.gd mesh-only/godot/world_hud.gd
for project in hud-only mesh-only; do
 "$GODOT_BIN" --headless --path "$project/godot" --editor --import --quit > "$project.import.log" 2>&1
 printf '0\n' > "$project.import.exit.txt"
done
python3 verify-extra.py
run hud-only hud-only 1200
run mesh-only mesh-only 1200
printf 'COMPLETE\n' > results/extra.complete
