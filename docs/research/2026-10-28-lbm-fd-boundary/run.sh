#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
exp=docs/research/2026-10-28-lbm-fd-boundary
export LP_NUM_THREADS=2 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1
"$exp/build.sh" > "$exp/results/build.log" 2>&1
{
 date -u '+%Y-%m-%dT%H:%M:%SZ'
 git rev-parse HEAD
 rustc --version
 python3 -c 'import numpy,scipy; print("numpy",numpy.__version__,"scipy",scipy.__version__)'
} > "$exp/results/environment.txt"
python3 "$exp/src/eigen.py" > "$exp/results/spectrum.log"
LBMDIAG=3 "$exp/bin/eigen_gpu" > "$exp/results/eigen-baseline.log" 2>&1
LBMDIAG=7 "$exp/bin/eigen_gpu" > "$exp/results/eigen-fd.log" 2>&1
LBMDIAG=7 "$exp/bin/eigen_gpu" coarse > "$exp/results/eigen-fd-coarse.log" 2>&1
LBMDIAG=8 "$exp/bin/diag" forced > "$exp/results/forced-moments.log" 2>&1
LBMDIAG=8 "$exp/bin/geometry" > "$exp/results/diagonal-stencil.log" 2>&1
# Two independent cases at a time, each with at most two llvmpipe worker threads.
LBMDIAG=8 "$exp/bin/diag" > "$exp/results/straight-fine.log" 2>&1 &
fine_pid=$!
LBMDIAG=8 "$exp/bin/diag" coarse > "$exp/results/straight-coarse.log" 2>&1 &
coarse_pid=$!
wait "$fine_pid"
wait "$coarse_pid"
LBMDIAG=8 "$exp/bin/pure" fork > "$exp/results/fork-fine.log" 2>&1 &
fine_pid=$!
LBMDIAG=8 "$exp/bin/pure" fork coarse > "$exp/results/fork-coarse.log" 2>&1 &
coarse_pid=$!
wait "$fine_pid"
wait "$coarse_pid"
python3 "$exp/analyze.py" > "$exp/results/summary.log"
(cd "$exp" && sha256sum results/*.log results/*.json results/*.bin results/environment.txt) > "$exp/results/results-manifest.sha256"
