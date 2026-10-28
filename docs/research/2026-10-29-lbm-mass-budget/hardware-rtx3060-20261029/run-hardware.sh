#!/usr/bin/env bash
set -euo pipefail
cd /workspace/lbm-mass-budget-20261029
export LBMDIAG=12 LSQ_WEIGHTS="$PWD/fork_a-fine.geom.weights"
export XDG_RUNTIME_DIR=/tmp/lbm-runtime
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
sha256sum -c input.sha256 > results/input-verification.txt
date -u +%Y-%m-%dT%H:%M:%SZ > results/started-utc.txt
set +e
./mass_budget --frames 500 --prefix-control 3 --output results/budget > results/run.log 2>&1
status=$?
set -e
printf '%s\n' "$status" > results/exit-code.txt
date -u +%Y-%m-%dT%H:%M:%SZ > results/finished-utc.txt
find results -type f ! -name results.sha256 -print0 | sort -z | xargs -0 sha256sum > results/results.sha256
exit "$status"
