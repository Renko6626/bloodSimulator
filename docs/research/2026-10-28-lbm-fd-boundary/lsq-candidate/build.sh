#!/usr/bin/env bash
set -euo pipefail
cd "$(git rev-parse --show-toplevel)/docs/research/2026-10-28-lbm-fd-boundary/lsq-candidate"
deps=${LBM_DEPS:-/data/sunyunbo/www/bloodSimulator/target/release/deps}
mkdir -p bin
for name in geometry eigen_gpu; do
 rustc --edition=2024 -O "src/$name.rs" -L "dependency=$deps" --extern "wgpu=$deps/libwgpu-5a7ee29f61bc02e0.rlib" --extern "bytemuck=$deps/libbytemuck-40ae7fd0291dbddd.rlib" --extern "pollster=$deps/libpollster-17ad20f61a6070c7.rlib" --extern "blood_fluid_gpu=$deps/libblood_fluid_gpu-2c48bc0f8179894e.rlib" --extern "simulation=$deps/libsimulation-b9a5802a0c6a13dc.rlib" -o "bin/$name"
done
sha256sum src/*.rs src/lbm/* weights.py build.sh run.sh summarize.py bin/* > results/build-manifest.sha256
