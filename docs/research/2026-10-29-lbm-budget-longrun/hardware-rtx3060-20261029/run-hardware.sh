#!/usr/bin/env bash
set -euo pipefail
cd /workspace/lbm-budget-longrun-20261029
export LBMDIAG=12 LSQ_WEIGHTS="$PWD/fork_a-fine.geom.weights"
export XDG_RUNTIME_DIR=/tmp/lbm-budget-runtime
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
sha256sum -c input.sha256 > results/input-verification.txt
date -u +%Y-%m-%dT%H:%M:%SZ > results/bench-started-utc.txt
set +e
timeout 180 ./budget_bench --warmup 100 --frames 500 --output results/bench > results/bench.log 2>&1
bench_status=$?
set -e
printf '%s\n' "$bench_status" > results/bench-exit-code.txt
date -u +%Y-%m-%dT%H:%M:%SZ > results/bench-finished-utc.txt
if [ "$bench_status" -eq 0 ]; then
 date -u +%Y-%m-%dT%H:%M:%SZ > results/long-started-utc.txt
 set +e
 timeout 1800 ./mean_density --frames 10000 --output results/long > results/long.log 2>&1
 run_status=$?
 set -e
 printf '%s\n' "$run_status" > results/long-exit-code.txt
 date -u +%Y-%m-%dT%H:%M:%SZ > results/long-finished-utc.txt
else
 run_status=$bench_status
fi
nvidia-smi --query-gpu=name,driver_version,memory.total,pstate,utilization.gpu --format=csv > results/gpu-after.csv
cat /proc/loadavg > results/loadavg-after.txt
find results -type f ! -name results.sha256 -print0 | sort -z | xargs -0 sha256sum > results/results.sha256
exit "$run_status"
