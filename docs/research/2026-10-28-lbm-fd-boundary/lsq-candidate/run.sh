#!/usr/bin/env bash
# Reproduction only: this script is not a request to rerun completed experiments.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
exp=docs/research/2026-10-28-lbm-fd-boundary/lsq-candidate
export LP_NUM_THREADS=2 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1
"$exp/build.sh" > "$exp/results/build.log" 2>&1
# Chosen cases only; each failed rank/manufactured/finite-state check stops the script.
for item in diagonal45-fine diagonal45-coarse fork_a-coarse fork_b-coarse fork_a-fine; do
 name=${item%-*}
 profile=${item##*-}
 args=("$name")
 if [ "$profile" = coarse ]; then args+=(coarse); fi
 LBMDIAG=12 "$exp/bin/geometry" "${args[@]}" --export "$exp/results/$item.geom" > "$exp/results/$item-export.log"
 python3 "$exp/weights.py" "$exp/results/$item.geom" > "$exp/results/$item-weights.log"
 LBMDIAG=12 LSQ_WEIGHTS="$exp/results/$item.geom.weights" "$exp/bin/geometry" "${args[@]}" --manufactured > "$exp/results/$item-manufactured.log" 2>&1
 LBMDIAG=12 LSQ_WEIGHTS="$exp/results/$item.geom.weights" "$exp/bin/geometry" "${args[@]}" > "$exp/results/$item.log" 2>&1
done
LBMDIAG=11 LSQ_EXPORT="$exp/results/strip.geom" "$exp/bin/eigen_gpu" > "$exp/results/strip-export.log"
python3 "$exp/weights.py" "$exp/results/strip.geom" > "$exp/results/strip-weights.log"
LBMDIAG=11 LSQ_WEIGHTS="$exp/results/strip.geom.weights" "$exp/bin/eigen_gpu" > "$exp/results/eigen-lsq.log" 2>&1
LBMDIAG=7 "$exp/bin/eigen_gpu" > "$exp/results/eigen-old-fd.log" 2>&1
python3 "$exp/summarize.py" > "$exp/results/summary.log"
