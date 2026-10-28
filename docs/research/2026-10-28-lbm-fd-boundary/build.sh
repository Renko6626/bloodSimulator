#!/usr/bin/env bash
set -euo pipefail
# Reuses only external dependencies; numerical module and embedded WGSL rebuild each invocation.
root=$(git rev-parse --show-toplevel)
exp="$root/docs/research/2026-10-28-lbm-fd-boundary"
deps=${LBM_DEPS:-/data/sunyunbo/www/bloodSimulator/target/release/deps}
mkdir -p "$exp/bin"
for name in eigen_gpu diag pure geometry; do
 rustc --edition=2024 -O "$exp/src/$name.rs" -L "dependency=$deps" --extern "wgpu=$deps/libwgpu-5a7ee29f61bc02e0.rlib" --extern "bytemuck=$deps/libbytemuck-40ae7fd0291dbddd.rlib" --extern "pollster=$deps/libpollster-17ad20f61a6070c7.rlib" --extern "blood_fluid_gpu=$deps/libblood_fluid_gpu-2c48bc0f8179894e.rlib" --extern "simulation=$deps/libsimulation-b9a5802a0c6a13dc.rlib" -o "$exp/bin/$name"
done
cd "$exp"
sha256sum src/*.rs src/*.py src/lbm/* bin/* analyze.py build.sh run.sh > results/build-manifest.sha256
sha256sum "$deps"/libwgpu-5a7ee29f61bc02e0.rlib "$deps"/libbytemuck-40ae7fd0291dbddd.rlib "$deps"/libpollster-17ad20f61a6070c7.rlib "$deps"/libblood_fluid_gpu-2c48bc0f8179894e.rlib "$deps"/libsimulation-b9a5802a0c6a13dc.rlib > results/dependency-manifest.sha256
