# Rayon 规则通道 Stokes 求解核实验

2026-10-26。从 [`2026-10-26-stokes-feedback/solver`](../../2026-10-26-stokes-feedback/solver/) 复制的独立研究 crate；旧实验、游戏源码及正式依赖均未改动。新 crate 唯一新增依赖为本机缓存的 `rayon = 1.12.0`。网格、有限阻力模型、Schur/marker CG、容差和最终残差计算保持原样。

## 接口与并行范围

`Stokes::new(n)` 和 `Stokes::with_threads(n, 0)` 使用原行主序内存布局及串行 FFT/Thomas 循环。`Stokes::with_threads(n, 1|2|4)` 在构造时建立并持有一个固定 Rayon 线程池，初始化成本应单列统计，不计入稳态 `solve`。不使用全局池，也不在每次求解时新建池。

并行路径只改规则通道速度逆 `A⁻¹`：每个 x 向 FFT 行独立运行；Fourier 模式原来在行主序数组中跨行跳跃，先转成每模式连续的独立工作数组，再对各模式做 y 向 Thomas 前代/回代，最后转回行主序。转置也通过互不重叠的切片并行；每个模式内部算术顺序与串行原核相同。没有 `unsafe` 或逐格锁。压力 Schur CG、有限阻力 marker CG、核准备、插值/散布、残差诊断和机械模拟仍串行。Rayon1 用来观察新内存布局、转置与调度成本。

两条路径都求完整稳态 Stokes/有限阻力自洽问题，均非单次投影。周期 x、固壁 y、μ=1 的模型与旧实验一致；不适用于不规则血管边界。`solve_lagged` 也沿用同一个速度逆，但其一次更新仍非自洽解。

## 真实数值核对

`numerical_check.rs` 用 4×4 个细胞、每细胞四点、每点 drag=0.0625、各细胞有不同规定 x/y 速度、背景体力 `(0.4,-0.1)`，在 32²、64²、128² 分别比较原串行与 Rayon1/2/4。完整原始输出在 [`results/numerical_check.csv`](results/numerical_check.csv)。9 组的网格速度、压力、点力、点上采样速度均逐 bit 相同；流体调用数、marker CG 与压力 CG 迭代数也相同。最大阻力律 RMS `1.573e-11`，最大散度 RMS `3.054e-11`，最大动量 RMS `3.795e-13`。这些是所测输入的观察结果，不保证其他编译目标逐 bit 一致。

本目录不记录正式性能数据。完整静态/动态对照由同轮统一计时完成，避免与其他 agent 争抢指定 CPU。Rayon 仅覆盖速度逆的行和列任务；即使该部分加速，也不能据此推算整个游戏步达到 5 ms。

## 复现命令

从仓库根目录运行；构建产物始终放在 `/tmp/sunyunbo/blood-stokes-rayon-target`：

```bash
cargo check --offline --manifest-path docs/research/experiments/2026-10-26-stokes-rayon/solver/Cargo.toml --target-dir /tmp/sunyunbo/blood-stokes-rayon-target
cargo run --release --offline --manifest-path docs/research/experiments/2026-10-26-stokes-rayon/solver/Cargo.toml --target-dir /tmp/sunyunbo/blood-stokes-rayon-target --bin numerical_check > docs/research/experiments/2026-10-26-stokes-rayon/solver/results/numerical_check.csv
cargo fmt --check --manifest-path docs/research/experiments/2026-10-26-stokes-rayon/solver/Cargo.toml
git diff --check
```

依赖由 `--offline` 成功解析，无网络安装。以上是数值验证命令，不是性能计时命令。
