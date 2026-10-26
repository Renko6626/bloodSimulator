# 64细胞密集场景

结果见[总报告](../../2026-10-26-stokes-64-cells.md)：64² 网格、64 个半径 0.045 的软体，8×8 布局，单体材料与 Γ=0.25 不变。一次更新方案在基线/优化、串行/四线程全部第16帧失败。已有自洽求解的四线程对照完成360帧，但整步均值约40.92ms。

本目录的 `dynamics/` 引用上一轮 `2026-10-26-stokes-hotpaths/{baseline,solver}`，不改写旧源码或归档。`optimized` Cargo feature 选择优化实现，场景其余参数保持一致。新的64布局在单位域内增加密度，未缩小细胞。

在仓库根目录复现：

```bash
# 预期第16帧失败；四组第一轮保存后停止，不运行第二轮计时
python3 docs/research/experiments/2026-10-26-stokes-64-cells/run.py
# 准确求解诊断对照，单条360帧轨迹
python3 docs/research/experiments/2026-10-26-stokes-64-cells/run.py --reference
# 冻结记录的第1帧几何，排除Rapier/接触，验证反馈放大
python3 docs/research/experiments/2026-10-26-stokes-64-cells/run_frozen.py
python3 docs/research/experiments/2026-10-26-stokes-64-cells/analyze.py
python3 docs/research/experiments/2026-10-26-stokes-64-cells/plot.py
```

编译输出在 `/tmp/sunyunbo/blood-stokes-rayon-target`。所有运行使用相同文件锁；串行CPU2，四线程CPU2–5，同插槽不同物理核。主机非独占。

`results/` 保存失败案例、日志和来源哈希；`reference-results/` 保存自洽对照；`frozen-results/` 保存冻结几何诊断、输入哈希和命令。`analyze.py` 不把失败前的部分帧计作有效性能结果。冻结诊断的比值对应每次 lagged 更新，不是每帧（动态每帧两个子步）。静态参考的零速度由真实求解器结果断言，未使用占位数据。

本轮未更改阻力、时间步、材料、求解阈值或新增稳定化机制，没有接入正式游戏。
