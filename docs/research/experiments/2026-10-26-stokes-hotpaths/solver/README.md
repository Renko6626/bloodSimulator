# Stokes 热路径独立实验

2026-10-26。此目录只实验上一轮 MAC/Stokes + 有限阻力 + Rayon 的实现成本，不接入正式游戏。包名 `stokes_hotpath_solver`。唯一外部算法依赖仍是 Rayon 1.12.0；可选 `comparison` feature 引用本地 `baseline_stokes_solver`，只供跨 crate 数值探针使用，常规构建不启用。

## 改动

- `Stokes::solve` 或 `solve_lagged` 调用开始时进入已建好的固定 Rayon pool 一次，整个流场更新中的并行任务都在该 pool 内。没有全局池、逐次建池或互斥锁。`threads=0` 保持串行；1/2/4 线程仍由 `Stokes::with_threads` 构造。
- FFT 实部/虚部及转置的 Fourier 模式工作数组在构造时分配，后续每次速度逆复用；输入复制到复用数组、虚部清零，结果仍完整拷到调用方输出。串行路径仍用原行主序和不转置的逐模式 Thomas 计算。
- 并行模式的同一任务连续执行 gather、Thomas 前代/回代，再按行 scatter。FFT 行与模式任务各用 `.with_min_len(4)` 粗化；每个模式和每行内的算术顺序未变。u/v 模式仍分两批，不宣称已消除全部调度开销。
- 单次完整 Stokes 解内复用 `apply_s` 的梯度、速度临时数组，最终压力梯度也复用该梯度数组。已有上帧场的 `solve_lagged` 直接借用场切片采样，不再全量 `to_vec`。
- 点力散布、Schur/marker CG 的 dot 与向量更新、梯度/散度 stencil 仍串行；没有改变物理方程、容差、迭代停止条件或诊断。未引入 RustFFT、其他算法依赖或较复杂的缓存框架。

`Marker` 和 `Solution` 的字段保持 baseline 一致。两个 crate 都支持 `set_profiling(bool)` 与 `take_profile() -> Profile`，字段和口径见 [baseline 说明](../baseline/README.md)。默认 profiling 关。`velocity_inverse_ms` 是包含准备、FFT、模式和拷贝的总 wall；其子项不能再加到总值上。`pressure_stencil_ms` 是梯度/散度的一部分，未覆盖所有 Schur 向量运算。`coupled_solve_ms` 与这些 Profile 时间嵌套，不能相加。正式性能结论须用 profiling 关闭的计时，开启的少量帧只用于定位。

## 数值核对与复现

[`results/compare_baseline.csv`](results/compare_baseline.csv) 是同一非平凡 16 细胞四点输入、背景体力 `(0.4,-0.1)` 的实际解：32/64/128² × 0/1/2/4 线程 × profiling 开/关，共 24 个自洽对照；另有 64²、0/2 线程、profiling 开/关共 4 个已有场 lagged 对照。所有对照的网格速度、压力、点力、点采样速度和最终残差逐 bit 相同，最大差为 0；fluid/marker/pressure 迭代计数一致，profile 开时速度逆调用数一致。此结论仅对所测编译环境与输入成立。数值 probe 同时检查计时字段开启时确有记录、关闭时逆调用计数为零；不会用计时数据作速度结论。

从仓库根目录运行：

```bash
cargo check --offline --manifest-path docs/research/experiments/2026-10-26-stokes-hotpaths/baseline/Cargo.toml --target-dir /tmp/sunyunbo/blood-stokes-rayon-target
cargo check --offline --features comparison --manifest-path docs/research/experiments/2026-10-26-stokes-hotpaths/solver/Cargo.toml --target-dir /tmp/sunyunbo/blood-stokes-rayon-target
cargo run --release --offline --features comparison --manifest-path docs/research/experiments/2026-10-26-stokes-hotpaths/solver/Cargo.toml --target-dir /tmp/sunyunbo/blood-stokes-rayon-target --bin compare_baseline > docs/research/experiments/2026-10-26-stokes-hotpaths/solver/results/compare_baseline.csv
cargo fmt --check --manifest-path docs/research/experiments/2026-10-26-stokes-hotpaths/baseline/Cargo.toml
cargo fmt --check --manifest-path docs/research/experiments/2026-10-26-stokes-hotpaths/solver/Cargo.toml
git diff --check
```

构建产物放在 `/tmp/sunyunbo/blood-stokes-rayon-target`。本目录无正式性能基准；静态/动态测量由本轮统一串行执行。规则周期通道的 FFT/Thomas 逆不能代表带分岔或复杂固壁的血管性能。
