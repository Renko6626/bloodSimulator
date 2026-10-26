# Rayon 并行 Stokes 实验

用户在 2026-10-26 批准仅在独立实验中引入 Rayon 常驻线程池。本轮接续[有限阻力反馈实验](../../2026-10-26-stokes-feedback.md)，保留其方程、容差、软体和输入，研究并行化是否改善一次更新 Stokes 的整步耗时。正式游戏与旧实验源码、结果保持原状。

已完成 16 条正式轨迹、3840 帧。结果与限制见[总报告](../../2026-10-26-stokes-rayon.md)；`results/summary.json` 保留每轮和合并统计，`review/` 保留独立复核。

## 实验范围

- `solver/` 复制旧规则通道求解器。`threads=0` 保留原行主序串行路径；1/2/4 创建固定线程数的私有 Rayon pool，常驻到求解器销毁。
- 并行行 FFT、独立 Fourier 模式的 Thomas 求解及模式数据的转置；压力 CG、点力、归约和 Rapier 保持串行。Rayon1 同时测到转置、分配及调度的代价。
- `dynamics/` 沿用上一轮真实 Rapier harness，只增加线程数选择和单独的 solver 初始化计时。使用 lagged 模式，每帧两个子步，每个子步一次完整 Stokes 更新。
- 同一单位域内 16 个半径 0.045 的 12 节点细胞，四个内部阻力点、总 Γ=0.25，两个细胞受 0.18 的 x 向外力持续 0.5 秒。64² 主测 360 帧，128² 短测 120 帧。
- 每个网格正序 0/1/2/4、反序 4/2/1/0 各一轮；基准互斥锁保证本轮任务间不并行计时，主机仍非独占。原串行和 Rayon1 使用 CPU2，Rayon2 使用 CPU2,3，Rayon4 使用 CPU2,3,4,5，均为同一插槽不同物理核。worker 继承进程 affinity，没有逐 worker 绑核。

## 验证与计时

`solver/numerical_check.rs` 比较 32/64/128² 非平凡受力问题的串行与 Rayon1/2/4，核对速度、压力、点力、采样速度全数组及真实残差。动态分析核对所有非计时 frame 字段，以及完整的 `cells.csv`、`outlines.csv`、`fields.csv` 与同网格串行输出一致；64² 原串行轨迹另对比旧实验归档。

整步 `whole_ms` 包括点构造、流体、反冲量回传、两个 Rapier 子步和诊断，排除初始化、CSV/快照输出与绘制。每条轨迹去掉前 30 帧，再统计均值、nearest-rank p95 和最大值，同时保留所有帧最大值。两轮合并 p95 由全部保留帧重新计算，不能平均两轮 p95。`init.csv` 单列求解器构造时间；进程 CPU/wall 时间还含初始化和文件输出，不能当作纯物理核成本。

## 运行

从仓库根目录执行：

```bash
python3 docs/research/experiments/2026-10-26-stokes-rayon/run.py --smoke --out /tmp/sunyunbo/stokes-rayon-smoke
python3 docs/research/experiments/2026-10-26-stokes-rayon/run.py
python3 docs/research/experiments/2026-10-26-stokes-rayon/analyze.py
```

`run.py` 使用离线锁定 release 构建，编译输出位于 `/tmp/sunyunbo/blood-stokes-rayon-target`；仅两个独立研究 crate 的 Cargo 锁文件包含 Rayon。正式结果记录命令、源文件哈希、CPU 拓扑、运行顺序和各条轨迹，重跑会覆盖同名结果。

此实验只衡量规则周期通道和当前弱阻力短轨迹。它不验证不规则血管墙、近壁标记、开口边界、百细胞负载或正式游戏帧预算。
