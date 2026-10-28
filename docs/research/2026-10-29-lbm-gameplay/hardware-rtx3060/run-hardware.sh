#!/usr/bin/env bash
set -euo pipefail
cd /workspace/lbm-gameplay-20261029
export XDG_RUNTIME_DIR=/tmp/lbm-gameplay-runtime
export DISPLAY=:99
mkdir -p "$XDG_RUNTIME_DIR" results
chmod 700 "$XDG_RUNTIME_DIR"

run() {
    local name=$1
    shift
    date -u +%Y-%m-%dT%H:%M:%SZ > "results/$name.started.txt"
    set +e
    timeout 600 "$@" > "results/$name.log" 2>&1
    local code=$?
    set -e
    printf '%s\n' "$code" > "results/$name.exit.txt"
    date -u +%Y-%m-%dT%H:%M:%SZ > "results/$name.finished.txt"
    return "$code"
}

mode=${1:?choose setup, smoke, probe, godot, controls, or vblank-control}
if [[ "$mode" == setup ]]; then
    sha256sum -c input/manifest.sha256 > results/input-verification.txt
    mkdir -p source
    tar -xzf input/source.tar.gz -C source
    mkdir -p source/target/debug source/target/release
    cp input/libgodot_bridge.so source/target/release/libgodot_bridge.so
    # Editor Godot requests the debug feature tag; both tags must load the
    # exact same release library in this experiment. Source manifest unchanged.
    ln -sf ../release/libgodot_bridge.so source/target/debug/libgodot_bridge.so
    nvidia-smi --query-gpu=name,driver_version,memory.total,pci.bus_id --format=csv > results/gpu.csv
    lscpu > results/cpu.txt
    getconf GNU_LIBC_VERSION > results/glibc.txt
    uname -a > results/uname.txt
    glxinfo -B > results/glxinfo.txt
    /workspace/godot-4.7.1 --version > results/godot-version.txt
    sha256sum /workspace/godot-4.7.1 source/target/debug/libgodot_bridge.so > results/runtime-binary.sha256
    run import /workspace/godot-4.7.1 --headless --path source/godot --editor --import --quit
elif [[ "$mode" == smoke ]]; then
    run probe-smoke input/lbm_gameplay_probe --case both --warmup 30 --frames 240 --output results/probe-smoke
    run godot-smoke /workspace/godot-4.7.1 --audio-driver Dummy --fixed-fps 60 --disable-vsync --path source/godot -- \
        --local-lbm --lbm-validation --lbm-benchmark=move --lbm-benchmark-warmup=120 --lbm-benchmark-frames=300 \
        --lbm-benchmark-output=/workspace/lbm-gameplay-20261029/results/godot-smoke.csv \
        --lbm-screenshot=/workspace/lbm-gameplay-20261029/results/gameplay.png
elif [[ "$mode" == probe ]]; then
    run probe input/lbm_gameplay_probe --case both --warmup 120 --frames 3600 --output results/probe
elif [[ "$mode" == godot ]]; then
    for scene in hold move; do
        extra=()
        if [[ "$scene" == hold ]]; then
            extra=(--lbm-screenshot=/workspace/lbm-gameplay-20261029/results/gameplay-validated.png)
        fi
        run "godot-$scene" /workspace/godot-4.7.1 --audio-driver Dummy --fixed-fps 60 --disable-vsync --path source/godot -- \
            --local-lbm --lbm-validation --lbm-benchmark="$scene" --lbm-benchmark-warmup=120 --lbm-benchmark-frames=3600 \
            --lbm-benchmark-output="/workspace/lbm-gameplay-20261029/results/godot-$scene.csv" "${extra[@]}"
    done
elif [[ "$mode" == controls ]]; then
    # Added only after the actual GL run exposed a large timing difference.
    # Same frozen code/physics and first 600 measured frames; no visual claim.
    run godot-headless-move /workspace/godot-4.7.1 --headless --fixed-fps 60 --path source/godot -- \
        --local-lbm --lbm-validation --lbm-benchmark=move --lbm-benchmark-warmup=120 --lbm-benchmark-frames=600 \
        --lbm-benchmark-output=/workspace/lbm-gameplay-20261029/results/godot-headless-move.csv
    run godot-no-draw-move /workspace/godot-4.7.1 --audio-driver Dummy --fixed-fps 60 --disable-vsync --disable-render-loop --path source/godot -- \
        --local-lbm --lbm-validation --lbm-benchmark=move --lbm-benchmark-warmup=120 --lbm-benchmark-frames=600 \
        --lbm-benchmark-output=/workspace/lbm-gameplay-20261029/results/godot-no-draw-move.csv
elif [[ "$mode" == vblank-control ]]; then
    # Godot --disable-vsync does not override a driver-level enforcement.
    # One process-local control only; no driver/global clock setting is changed.
    run godot-vblank-off-move env __GL_SYNC_TO_VBLANK=0 /workspace/godot-4.7.1 --audio-driver Dummy --fixed-fps 60 --disable-vsync --path source/godot -- \
        --local-lbm --lbm-validation --lbm-benchmark=move --lbm-benchmark-warmup=120 --lbm-benchmark-frames=600 \
        --lbm-benchmark-output=/workspace/lbm-gameplay-20261029/results/godot-vblank-off-move.csv
else
    echo "unknown mode: $mode" >&2
    exit 2
fi
