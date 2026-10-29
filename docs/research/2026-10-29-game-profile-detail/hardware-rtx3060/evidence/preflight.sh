#!/usr/bin/env bash
set -euo pipefail
cd /workspace/blood-game-profile-detail
export DISPLAY=:99 XDG_RUNTIME_DIR=/tmp/blood-profile-detail-runtime
export LD_LIBRARY_PATH="/workspace/blood-game-e2e/display/glvnd/usr/lib/x86_64-linux-gnu:/workspace/blood-game-e2e/display/driver:${LD_LIBRARY_PATH:-}"
mkdir -p "$XDG_RUNTIME_DIR" results
chmod 700 "$XDG_RUNTIME_DIR"
GODOT_BIN=/workspace/blood-game-e2e/godot-bin
nvidia-smi --query-gpu=name,uuid,driver_version,pstate,clocks.current.graphics,clocks.current.memory --format=csv > results/gpu-before.csv
nvidia-smi --query-gpu=timestamp,pstate,clocks.current.graphics,clocks.current.memory,utilization.gpu,memory.used,power.draw --format=csv -lms 200 > results/gpu-timeline.csv &
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
run preflight instrumented 30 BLOOD_PROFILE="$PWD/results/preflight" BLOOD_GODOT_GPU_PROFILE=1 BLOOD_GPU_PROFILE=1
