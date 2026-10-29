#!/usr/bin/env bash
set -euo pipefail
cd /workspace/blood-game-e2e
export DISPLAY=:99 XDG_RUNTIME_DIR=/tmp/blood-e2e-runtime
mkdir -p "$XDG_RUNTIME_DIR" results
chmod 700 "$XDG_RUNTIME_DIR"
GODOT_BIN="$PWD/godot-bin"
export LD_LIBRARY_PATH="$PWD/display/glvnd/usr/lib/x86_64-linux-gnu:$PWD/display/driver:${LD_LIBRARY_PATH:-}"
# No forced performance clocks: observe the actual context and GPU state.
nvidia-smi --query-gpu=name,uuid,driver_version,pstate,clocks.current.graphics,clocks.current.memory --format=csv > results/gpu-before.csv
lscpu > results/cpu.txt
cat /sys/fs/cgroup/cpu.max > results/cpu-quota.txt 2>/dev/null || true
nvidia-smi --query-gpu=timestamp,pstate,clocks.current.graphics,clocks.current.memory,utilization.gpu,memory.used,power.draw --format=csv -lms 200 > results/gpu-timeline.csv &
monitor_pid=$!
trap 'kill "$monitor_pid" 2>/dev/null || true' EXIT
run() {
    local name=$1 project=$2 frames=$3 mode=$4
    shift 4
    date -u +%FT%TZ > "results/$name.started.txt"
    set +e
    timeout --kill-after=10 600 "$GODOT_BIN" --audio-driver Dummy --fixed-fps 60 --disable-vsync --path "$project/godot" -- \
        --lbm-benchmark="$mode" --lbm-benchmark-warmup=120 --lbm-benchmark-frames="$frames" \
        --lbm-benchmark-output="$PWD/results/$name.csv" --lbm-screenshot="$PWD/results/$name.png" "$@" > "results/$name.log" 2>&1
    local code=$?
    set -e
    printf '%s\n' "$code" > "results/$name.exit.txt"
    date -u +%FT%TZ > "results/$name.finished.txt"
    printf '%s exit=%s\n' "$name" "$code" | tee -a results/runner-status.txt
}
# All primary LBM cases use the unmodified existing product benchmark.
run baseline-lbm baseline 600 move --local-lbm --lbm-validation
run current-lbm-full current 3600 move --local-lbm --lbm-validation
run current-lbm-local current 3600 move --local-lbm --lbm-validation --local-cells
# A diagnostic-only extension of the same benchmark permits the default map hold.
run current-world-full world 600 hold
run current-world-local world 600 hold --local-cells
nvidia-smi --query-gpu=name,pstate,clocks.current.graphics,clocks.current.memory --format=csv > results/gpu-after.csv
printf 'COMPLETE\n' > results/runner.complete
