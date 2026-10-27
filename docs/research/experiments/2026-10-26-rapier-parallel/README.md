# Rapier 接触距离与并行实验

2026-10-26。用户已确认调整参数并启用 Rapier `parallel`。本目录从上一轮欠松弛动态场景复制，使用原求解器的相对路径依赖，不改旧基线和正式游戏。

## 设置

`dynamics/` 默认启用本包的 `parallel` feature，映射到 `rapier2d/parallel`；`--no-default-features` 构建真实串行对照。Rapier 仍锁定 0.36.1，Rayon 仍为 1.12.0。

预测接触距离默认从 0.02 调为 0.002，由 `--prediction` 指定。`length_unit` 保持 1；细胞直径 0.09、粒子半径 0.0045、12 节点、膜 12 Hz、面积保持 30 Hz、默认额外 PGS 3 次和每帧两个机械子步均不变。不把这一参数直接用于直径 1.4 的正式世界。

`--rapier-threads` 默认 4，仅允许 1/2/4，初始化时明确设置 Rayon 全局池。已有流体求解器继续使用独立四线程池；流体和机械阶段依次执行，没有逐步建池。`init.csv` 与日志保存实际并行 feature、线程数和预测距离。未设置线程数时不会按服务器全部可用核心建池。

## 复现

仓库根目录执行：

```bash
python3 docs/research/experiments/2026-10-26-rapier-parallel/run.py
python3 docs/research/experiments/2026-10-26-rapier-parallel/analyze.py
```

主对照四组各运行两轮，第二轮反序；另做并行构建的单线程、双线程各一轮。每组 64²/64 细胞、ω=0.5、360 帧。release、offline、locked，CPU 2–5 同插槽四个物理核，共用历史实验文件锁。前 30 帧不计入耗时分位数，p95 使用 nearest rank；整步包含原有诊断，不含 CSV 输出、初始化、绘制。没有额外内部计时埋点。

`--case serial-tuned-r1 --frames 2 --out /tmp/...` 可做短运行，不覆盖归档结果。编译产物在 `/tmp/sunyunbo/blood-stokes-rayon-target`。`results/manifest.json` 保存全部命令、退出码、源码和二进制哈希；分析脚本复算统计并核对记录的轨迹及原欠松弛基线。

## 结果

见[研究报告](../../2026-10-26-rapier-parallel.md)及 [summary.json](results/summary.json)。此场景首选预测距离 0.002、Rapier 四线程；整步均值 6.34–7.10 ms，仍未达到 5 ms。六秒轨迹不能代表持续拥堵、高速碰撞或正式分叉地图。
