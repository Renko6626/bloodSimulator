#!/usr/bin/env bash
set -euo pipefail
cd /workspace/blood-cell-batch-rtx
export DISPLAY=:99 XDG_RUNTIME_DIR=/tmp/blood-cell-batch-runtime
export LD_LIBRARY_PATH="/workspace/blood-game-e2e/display/glvnd/usr/lib/x86_64-linux-gnu:/workspace/blood-game-e2e/display/driver:${LD_LIBRARY_PATH:-}"
timeout 60 /workspace/blood-game-e2e/godot-bin --path candidate/godot --fixed-fps 60 --disable-vsync --audio-driver Dummy -- \
 --local-lbm --lbm-validation --local-cells --lbm-benchmark=move --lbm-benchmark-warmup=120 --lbm-benchmark-frames=30 \
 --lbm-benchmark-output="$PWD/results/capture.csv" --lbm-screenshot="$PWD/results/candidate.png" > results/capture.log 2>&1
printf '0\n' > results/capture.exit.txt
