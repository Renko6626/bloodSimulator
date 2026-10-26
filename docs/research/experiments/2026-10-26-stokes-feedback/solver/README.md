# 有限阻力双向反馈：规则通道静态探针

日期：2026-10-26。此目录是独立研究 crate，不接入游戏，也不选择正式后端。`fluid.rs` 复用 2026-10-23 标记实验的 MAC 离散、完整压力 Schur 求解和原样 `separable.rs` FFT/Thomas 速度逆。单位方域，x 周期、y 上下无滑移，μ=1；没有流体时间惯性。流体与标记始终双向交换力，允许漏流。

## 算法与接口

`Marker { position, velocity, drag }` 的 `drag` 是每点阻力。`Stokes::solve` 在标记力空间用矩阵自由 CG 解

```text
(diag(1/drag) + J K S) F = V - J K f_background
```

其中 K 是完整稳态 Stokes 速度响应，每次矩阵作用都调用完整速度/压力求解；S 与 J 使用相同四点 Peskin 权重，散布除以 h²。最终再解一次完整流场，并直接检查 `F/drag + Ju - V`、`Au+Gp-f`、`Du`，因此没有把一次投影或滞后更新称为自洽解。`Solution.marker_forces` 是标记作用于流体的力，物体受其反力。

`Stokes::solve_lagged(markers, background_force, prior)` 是便宜对照：用 `prior` 场在当前标记点采样，算一次 F，然后做一次完整 Stokes 更新；`prior=None` 时先求无细胞背景。它的 `force_law_rms` 用**最终新场**验算，不满足自洽时会如实显示。后续动态实验可把前一机械子步的 `Solution` 传入 `prior`。已知靠墙核支持会被固壁截断；`solve` 和 `solve_lagged` 都拒绝这种标记位置，不做静默重归一化。`sample_at` 是内部同核插值，场导出改用 MAC 面到格心算术平均，墙上法向速度取零。

## 输入与原始产物

- 16、32、64 个细胞分别排成 4×4、8×4、8×8；每细胞四个标记在中心的四个轴向，offset=0.045，每点 drag=0.0625，总 Γ=0.25。所有标记共同耦合一个流场。
- `stationary`：标记速度零，背景体力 `(1,0)`；`moving`：标记速度 `(0.1,0)`，背景体力零。各场景均用相同背景力的无细胞解作对照。静态 offset 与动态实验的机械半径及“质心到节点中点”标记位置**不同**，这里不能作为动态初态的逐点复现。
- `results/samples.csv`：32²/16、64²/16、64²/32 各三次；128²/64 仅一次。`kernel_prepare_ms`、`coupled_solve_ms`、`diagnostics_ms`、`total_ms` 分列；`construct_ms` 单独计。128² 无 p95。原始数据由 release 单线程固定 CPU 2、`flock` 串行测得，主机不独占。
- `results/lagged_samples.csv`：16 细胞的 32²、64²，以同场景无细胞解作 prior，各一次。它是单次背景起步对照，不代表连续时间中的上帧场。
- `results/field_{stationary,moving}_n64_c16.csv`：64²/16 的无细胞和自洽场格心速度及差值；两种场共用同一几何。`results/one_cell_reference.csv` 与 `results/legacy_dense_reference.csv` 是单细胞 matrix-free 和旧 dense mobility 的同输入核对；旧场也保留在 `results/legacy-dense-fields/`。

## 数值与成本

| 场景 | 32²/16 总均值 | 64²/16 总均值 | 64²/32 总均值 | 128²/64 单次 |
| --- | ---: | ---: | ---: | ---: |
| 静止细胞、背景流 | 4.940 ms | 28.443 ms | 25.273 ms | 177.720 ms |
| 规定平移、无背景力 | 5.399 ms | 33.122 ms | 26.893 ms | 208.741 ms |

这些数值只包含静态流体求解、核准备和诊断，不含机械、接触、渲染或游戏逻辑。64²/32 比 64²/16 快来自**不同几何布局及对应压力迭代量**，不能推论增加细胞会提速。计时受非独占主机负载影响，不将均值或单样本解释为目标设备保证。

稳态背景流解析解为 `u(y)=y(1-y)/2`，无细胞离散解最大点误差在 32² 是 `1.221e-4`。本批自洽最终残差最大值：散度 RMS `2.381e-11`，动量 RMS `2.881e-13`，阻力律 RMS `3.399e-11`（速度单位），网格与标记合力差 `2.442e-15`。32²/16 静止场的流量从无细胞 `0.083496` 降到 `0.059838`；规定运动场从零升到 `0.024344`，64² 场 CSV 展示局部扰动。旧 dense mobility 单细胞 32²、半径 0.075、Γ=0.25 的物体反力 x=`-0.02351094083`；本 matrix-free 解的流体点合力 x=`+0.023510940832189`，符号与数值一致。

单次滞后更新虽只需一次完整 Stokes 求解，32²/16 的两场约 `1.04–1.07 ms`，但最终阻力律 RMS 为 `0.0264–0.0267`，约为规定运动速度 0.1 的 26%。静止场流量 `0.050169` 对自洽 `0.059838`；运动场 `0.034119` 对自洽 `0.024344`。它在本多细胞快照中不属于几%闭合误差的近似；连续动态中的上帧场要另看轨迹证据。

## 复现

从仓库根目录运行：

```bash
cargo build --release --offline --manifest-path docs/research/experiments/2026-10-26-stokes-feedback/solver/Cargo.toml --target-dir /tmp/blood-stokes-feedback-solver-target
flock /tmp/blood-stokes-feedback-bench.lock taskset -c 2 /tmp/blood-stokes-feedback-solver-target/release/static_probe docs/research/experiments/2026-10-26-stokes-feedback/solver/results
rustc -O docs/research/experiments/2026-10-23-coupling/markers/main.rs -o /tmp/blood-stokes-dense-reference
flock /tmp/blood-stokes-feedback-bench.lock taskset -c 2 /tmp/blood-stokes-dense-reference 32 4 docs/research/experiments/2026-10-26-stokes-feedback/solver/results/legacy-dense-fields --shape blob --drag 0.25 > docs/research/experiments/2026-10-26-stokes-feedback/solver/results/legacy_dense_reference.csv
cargo fmt --check --manifest-path docs/research/experiments/2026-10-26-stokes-feedback/solver/Cargo.toml
cargo check --offline --manifest-path docs/research/experiments/2026-10-26-stokes-feedback/solver/Cargo.toml --target-dir /tmp/blood-stokes-feedback-solver-target
```

环境、源码 SHA-256 和实际命令见 `results/environment.txt`。运行 probe 会覆盖本目录的原始 CSV。规则周期通道专用 FFT/Thomas 逆不能直接外推到分岔血管、障碍边界或动态近壁接触。32² 自洽成本已经接近或超过整个 5 ms 目标，64² 明显超过；它仍是比较其他后端所需的基线。
