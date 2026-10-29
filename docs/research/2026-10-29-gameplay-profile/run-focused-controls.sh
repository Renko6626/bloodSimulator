#!/usr/bin/env bash
# Run only after the primary matrix has completed. Diagnostic copies only.
set -euo pipefail
export GODOT_BIN="$PWD/godot-4.7.1" DISPLAY=:99 XDG_RUNTIME_DIR=/tmp/lbm-profile-runtime
cp -a instrumented no-aa
python3 - <<'PY'
from pathlib import Path
p=Path('no-aa/godot/world.gd')
s=p.read_text()
a=s.index('func _draw() -> void:')
b=s.index('func _cell_centers()',a)
draw=s[a:b]
assert draw.count(', true)')==8
p.write_text(s[:a]+draw.replace(', true)',', false)')+s[b:])
PY
run() {
    local name=$1 project=$2 frames=$3
    shift 3
    date -u +%FT%TZ > "results/$name.started.txt"
    nvidia-smi --query-gpu=timestamp,pstate,clocks.current.graphics,clocks.current.memory,utilization.gpu,power.draw --format=csv --loop-ms=200 -f "results/$name.gpu.csv" &
    local sampler=$!
    set +e
    BLOOD_PROFILE="$PWD/results/$name" BLOOD_GPU_PROFILE=1 timeout --kill-after=10 180 "$@" "$GODOT_BIN" --audio-driver Dummy --fixed-fps 60 --disable-vsync --path "$project/godot" -- \
        --local-lbm --lbm-validation --lbm-benchmark=move --lbm-benchmark-warmup=120 --lbm-benchmark-frames="$frames" --lbm-benchmark-output="$PWD/results/$name.csv" > "results/$name.log" 2>&1
    local code=$?
    kill "$sampler"
    wait "$sampler" 2>/dev/null
    set -e
    printf '%s\n' "$code" > "results/$name.exit.txt"
    date -u +%FT%TZ > "results/$name.finished.txt"
    return "$code"
}
run no-aa no-aa 600 env
# One repeated normal run is necessary to judge the magnitude/order dependence
# of the single-variable visual control, not a search for a best run.
run normal-repeat instrumented 600 env
# A short syscall control attributes blocking calls unavailable to the optional
# libc interposer (e.g. driver ioctls). Timing under ptrace is diagnostic only.
run syscall-trace instrumented 180 strace -f -tt -T -yy -o "$PWD/results/syscalls.log" -e trace=ioctl,futex,poll,ppoll,pselect6,nanosleep,clock_nanosleep,epoll_wait,sched_yield
