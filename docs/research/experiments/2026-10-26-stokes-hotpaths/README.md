# Stokes 热路径与粗粒度并行实验

日期：2026-10-26。用户批准接续 [Rayon 实验](../../2026-10-26-stokes-rayon.md)，先做分段计时、缓冲复用和粗粒度并行。本轮沿用同一流体方程、容差、弱阻力耦合与 Rapier 软体，不新增 RustFFT 或改变正式游戏后端。

已完成 32 条主性能轨迹（7680 帧）与 4 条独立 profiling 轨迹（480 帧）。结论、完整性能表与限制见[总报告](../../2026-10-26-stokes-hotpaths.md)。

## 对照组织

- `baseline/`：上一轮 Rayon 求解器，加默认关闭的可选细分计时。
- `solver/`：优化版，复用 FFT/模式工作数组，合并 gather 与 Thomas 求解，将线程池入口移到完整流体更新外层，减少 prior 场复制，在单次 Stokes 求解中复用 Schur 工作数组。
- `dynamics/`：沿用上一轮真实软体 harness，通过 Cargo feature `optimized` 在编译时选择求解器；物理输入和循环相同。
- `review/`：独立数值、并发和计时审计。

同一输入包含 16 个半径 0.045、12 节点的软体，四个内部流体点，总 Γ=0.25。两个细胞受到 x 向外力 0.18，持续 0.5 秒后撤力。单位方域，x 周期，上下平直固壁；每帧两个机械子步、60 Hz，均采用上一子步流场求力的一次更新 Stokes。

## 运行与计时

```bash
python3 docs/research/experiments/2026-10-26-stokes-hotpaths/run.py --smoke --out /tmp/sunyunbo/stokes-hotpaths-smoke
python3 docs/research/experiments/2026-10-26-stokes-hotpaths/run.py
python3 docs/research/experiments/2026-10-26-stokes-hotpaths/analyze.py
```

`run.py` 以离线锁定 release 构建两份独立二进制。原串行、Rayon1/2/4 分别使用 CPU2、CPU2、CPU2,3、CPU2,3,4,5；都是同一插槽的不同物理核。所有案例通过共用文件锁串行测量，主机非独占。

每个网格、线程数均比较 baseline/optimized，正序一轮后反序再一轮，版本顺序也反转。64² 每例 360 帧，128² 每例 120 帧。主结果明确关闭 profiling；每例剔除前 30 帧，整步计时含流体、回传、Rapier 和诊断，排除初始化与文件/图像输出。保留每轮结果、合并均值/p95、全部峰值和进程 CPU 消耗，不选最快一轮。

另外对 64² 的 baseline/optimized、串行/四线程各跑一条 120 帧 profiling 轨迹，仅用于定位。`profile.csv` 每个机械子步一行，`velocity_inverse_ms` 包括准备复制、正向 FFT、模式求解/转置、逆 FFT；这些子项不能再与 inverse 总量相加。`pressure_stencil_ms` 是压力相关梯度/散度等埋点覆盖的部分，不是完整压力迭代。profile 运行不混入主性能统计。

线程池入口在 baseline 的 velocity inverse 计时内部，在优化版移到了该计时外部，仍计入 fluid/whole。因此 inverse 总量及其未分类余数的下降不能全部解释为消除的成本；应以关闭 profiling 的整步对照判断净收益。

## 正确性边界

静态数值 probe 对比两个 crate 的真实完整求解和 prior-lagged 结果。动态分析检查所有非计时 frame 字段、逐细胞轨迹、轮廓与流场快照一致；64² baseline 串行轨迹另与上一轮归档核对。profiling 开关也必须保持数值一致。

只在当前规则通道、弱阻力短轨迹下判断优化收益；不代表不规则血管墙、开放窗口、近壁接触或百细胞规模已验证。代码中的 `.with_min_len(4)` 限制每个 Rayon 任务分割的最小元素数，不保证总任务数固定为线程数。
