# faer 速度矩阵实验

用户批准的独立实验：将六次采样、`1e-4` 压力容差及几何查询复用候选的速度矩阵求解替换为 faer。正式源码、Cargo 清单和锁文件未改。结论见[报告](../../2026-10-27-faer-speed.md)。

## 内容

- `linear.rs`：faer 0.24.4 低层稀疏 LLT 适配器，默认 AMD 与 AUTO 策略；每个分量显式 `Par::Seq`，外层保留四线程池中的 u/v join。
- `audit.rs`：同一真实分叉速度矩阵、两个分量各四个非平凡右端项，与原自写稀疏求解器比较，并从原 diagonal/neighbors 独立计算 `A*x-b`。
- `run.py`：继承[几何复用候选](../2026-10-27-geometry-query/run.py)，只在 `/tmp` 副本添加 faer。两变体使用相同的依赖和特性，验证原有锁定包版本均保留；分别保存新锁文件。
- `analyze.py`：核对源码哈希、正式文件未改、每条轨迹有限值与机械范围、同后端重放一致性；跨后端比较误差，不要求浮点位级相同。
- `results/`：六条原始轨迹、编译与测试日志、锁文件、环境、清单及统计。`FACTOR` 的两种字节口径不同，见报告；不得直接对比后宣称总因子内存减少一半。

## 复现

需要可联网下载依赖的 Rust 工具链、Python 3、Linux `flock` / `taskset` / `/usr/bin/time`，以及可用的 CPU 2–5。首次 faer 编译不计入模拟耗时。

```bash
python3 docs/research/experiments/2026-10-27-faer-speed/run.py --out /tmp/faer-replay
python3 docs/research/experiments/2026-10-27-faer-speed/analyze.py --results /tmp/faer-replay
```

原始结果拒绝覆盖。默认每条轨迹 30 模拟秒，1,800 帧；按 custom→faer、faer→custom 顺序运行两组，再分别启用 fluid 细分计时。固定四核、共享既有性能实验文件锁，未启用 faer Rayon 特性。计时是 `tick` 阶段，快照与 CSV 写出在计时外。细分计时与主计时分开，两个分量的累计工作时间不能当作并行墙钟时间。

已有严格流体检查只在测试构建中恢复 `2e-9`，保持原断言；动态构建恢复 `1e-4`。每种构建运行 3 项原有流体检查、原有带状解对照及新增自写稀疏对照，共 5 项。新增检查仅存在于实验副本，未新增正式永久测试。完整命令见 `results/manifest.json`，无需运行 Godot 或自动操作 UI。

格式与分析检查：

```bash
rustfmt --check --edition 2024 docs/research/experiments/2026-10-27-faer-speed/linear.rs docs/research/experiments/2026-10-27-faer-speed/audit.rs
python3 docs/research/experiments/2026-10-27-faer-speed/analyze.py
```
