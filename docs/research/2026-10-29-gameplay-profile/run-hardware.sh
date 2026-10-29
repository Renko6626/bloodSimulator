#!/usr/bin/env bash
# Run from a staging directory containing baseline/, instrumented/, binaries/.
# Same GL backend, geometry, material, input and 72 cells in every GPU control.
set -euo pipefail
export DISPLAY=${DISPLAY:-:99}
export XDG_RUNTIME_DIR=/tmp/lbm-profile-runtime
mkdir -p "$XDG_RUNTIME_DIR" results
chmod 700 "$XDG_RUNTIME_DIR"
GODOT_BIN=${GODOT_BIN:-/workspace/godot-4.7.1}
run() {
    local name=$1
    shift
    date -u +%FT%TZ > "results/$name.started.txt"
    set +e
    timeout --kill-after=10 180 "$@" > "results/$name.log" 2>&1
    local code=$?
    set -e
    printf '%s\n' "$code" > "results/$name.exit.txt"
    date -u +%FT%TZ > "results/$name.finished.txt"
    return "$code"
}
game() {
    local name=$1 project=$2
    shift 2
    run "$name" "$GODOT_BIN" --audio-driver Dummy --fixed-fps 60 --disable-vsync --path "$project/godot" "$@" -- \
        --local-lbm --lbm-validation --lbm-benchmark=move --lbm-benchmark-warmup=120 --lbm-benchmark-frames=600 \
        --lbm-benchmark-output="$PWD/results/$name.csv"
}
# Xorg startup/import are staging steps; confirm glxinfo uses NVIDIA before this.
nvidia-smi --query-gpu=name,uuid,driver_version,pstate,clocks.current.graphics,clocks.current.memory --format=csv > results/gpu-before.csv
lscpu > results/cpu.txt
cat /sys/fs/cgroup/cpu.max > results/cpu-quota.txt
sha256sum binaries/* > results/binaries.sha256
run baseline-probe binaries/baseline-probe --case move --warmup 120 --frames 600 --output results/baseline-probe
game baseline-godot baseline
export BLOOD_PROFILE=1
run detail-probe env BLOOD_GPU_PROFILE=1 binaries/detail-probe --case move --warmup 120 --frames 600 --output results/detail-probe
export BLOOD_PROFILE="$PWD/results/detail-godot"
game detail-godot instrumented
export BLOOD_GPU_PROFILE=1 BLOOD_PROFILE="$PWD/results/timestamps-godot"
game timestamps-godot instrumented
export BLOOD_PROFILE="$PWD/results/no-world" BLOOD_PROFILE_VISUAL=no-world
game no-world instrumented
export BLOOD_PROFILE="$PWD/results/no-hud" BLOOD_PROFILE_VISUAL=no-hud
game no-hud instrumented
unset BLOOD_PROFILE_VISUAL
export BLOOD_PROFILE="$PWD/results/no-draw"
game no-draw instrumented --disable-render-loop
export BLOOD_PROFILE="$PWD/results/headless"
game headless instrumented --headless
# Optional wait tracer is a separate bounded run, avoiding perturbation of the
# primary timing controls. Copy the compiled .so into binaries first.
if [[ -f binaries/wait-trace.so ]]; then
    export BLOOD_PROFILE="$PWD/results/wait-trace"
    export BLOOD_WAIT_TRACE="$PWD/results/waits.csv" LD_PRELOAD="$PWD/binaries/wait-trace.so"
    game wait-trace instrumented
    unset LD_PRELOAD BLOOD_WAIT_TRACE
fi
nvidia-smi --query-gpu=name,pstate,clocks.current.graphics,clocks.current.memory --format=csv > results/gpu-after.csv
