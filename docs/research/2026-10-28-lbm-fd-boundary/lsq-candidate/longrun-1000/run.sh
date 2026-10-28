#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
exp=docs/research/2026-10-28-lbm-fd-boundary/lsq-candidate
out="$exp/longrun-1000"
deps=${LBM_DEPS:-/data/sunyunbo/www/bloodSimulator/target/release/deps}
rustc --edition=2024 -O "$out/geometry.rs" -L "dependency=$deps" --extern "wgpu=$deps/libwgpu-5a7ee29f61bc02e0.rlib" --extern "bytemuck=$deps/libbytemuck-40ae7fd0291dbddd.rlib" --extern "pollster=$deps/libpollster-17ad20f61a6070c7.rlib" --extern "blood_fluid_gpu=$deps/libblood_fluid_gpu-2c48bc0f8179894e.rlib" --extern "simulation=$deps/libsimulation-b9a5802a0c6a13dc.rlib" -o "$out/geometry" 2> "$out/build.log"
sha256sum "$out/geometry.rs" "$out/geometry" "$out/run.sh" "$exp"/src/lbm/* "$exp/results/fork_a-fine.geom.weights" > "$out/source-binary.sha256"
export LP_NUM_THREADS=2 OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1
set +e
LBMDIAG=12 LSQ_WEIGHTS="$exp/results/fork_a-fine.geom.weights" "$out/geometry" fork_a > "$out/run.log" 2>&1
status=$?
printf '%s\n' "$status" > "$out/exit-code.txt"
exit "$status"
