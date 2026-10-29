#!/usr/bin/env bash
# Longer only to distinguish a short startup boost from sustained mode effects.
set -euo pipefail
export DISPLAY=:99 XDG_RUNTIME_DIR=/tmp/lbm-profile-runtime
run() {
    local name=$1
    shift
    date -u +%FT%TZ > "results/$name.started.txt"
    nvidia-smi --query-gpu=timestamp,pstate,clocks.current.graphics,clocks.current.memory,utilization.gpu,power.draw --format=csv --loop-ms=200 -f "results/$name.gpu.csv" &
    local sampler=$!
    set +e
    timeout --kill-after=10 240 "$@" > "results/$name.log" 2>&1
    local code=$?
    kill "$sampler"
    wait "$sampler" 2>/dev/null
    set -e
    printf '%s\n' "$code" > "results/$name.exit.txt"
    date -u +%FT%TZ > "results/$name.finished.txt"
    return "$code"
}
run no-world-long env BLOOD_PROFILE="$PWD/results/no-world-long" BLOOD_GPU_PROFILE=1 BLOOD_PROFILE_VISUAL=no-world "$PWD/godot-4.7.1" --audio-driver Dummy --fixed-fps 60 --disable-vsync --path instrumented/godot -- \
    --local-lbm --lbm-validation --lbm-benchmark=move --lbm-benchmark-warmup=120 --lbm-benchmark-frames=2400 --lbm-benchmark-output="$PWD/results/no-world-long.csv"
run probe-long env BLOOD_PROFILE=1 BLOOD_GPU_PROFILE=1 "$PWD/binaries/detail-probe" --case move --warmup 120 --frames 2400 --output results/probe-long
