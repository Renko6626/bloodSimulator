# 分叉 Stokes 稀疏速度分解实验

2026-10-26。用户批准实验 1：保留压力 Schur-CG，比较带状与最小度重排序的稀疏 Cholesky。随后根据用户关于游戏精度的提问，追加独立压力容差参数对照。正式工程没有更换求解器、增加依赖或改动 Godot 入口。

## 文件

- `baseline/`：测量前冻结的 workspace 清单、锁文件和 crates；包括 godot_bridge 以保持原锁文件有效，实验只构建 simulation。
- `sparse.rs`：确定性贪心最小度排序、符号填充、left-looking 稀疏 Cholesky 和前回代。沿用复制工程的 `BandCholesky` 类型名，仅为替换原内部接口；算法本身为稀疏实现，不是 SuiteSparse AMD。
- `compare.rs`：真实分叉、同一矩阵及同一右端的速度解与独立原方程残差对照；只注入临时研究构建，不增加正式永久测试。
- `run.py`：在 `/tmp/sunyunbo` 构建两种独立副本，启用同样的可选埋点和帧记录；运行四条匹配轨迹、两条独立 profiling 轨迹。
- `analyze.py`：核对测量源码哈希、逐帧数值与配置对照，复算耗时和因子统计。
- `tolerance.py`：在稀疏副本中仅改变压力停止容差，并记录全部开口的流入／流出；不修改实验 1 数据。
- `analyze_tolerance.py`：独立复算容差组的迭代、耗时、开口收支、细胞中心和采样速度变化，不套用原严格残差阈值声称验收通过。
- `longrun.py` / `longrun.rs`：用户要求的 120 秒长期对照，构造带有相同诊断的两个临时副本；逐帧记录所有子步峰值及通量积分，每秒采样细胞和流向。
- `replay_longrun.py`：发现轨迹分岔后补做同一严格二进制的 30 秒重放，控制重复运行差异。
- `analyze_longrun.py`：核对长时记录、原机械范围、宏观交通、累计连续性缺口、个体轨迹及重放结果。
- `results/`：命令、源码与二进制哈希、原始日志、逐帧 CSV、复算统计；后续容差结果位于 `results/tolerance/`。

## 复现

在仓库根目录运行。已归档目录存在 manifest 时，运行脚本拒绝覆盖；复测请指定新的临时目录。

```bash
# 实验 1 的原始测量命令（已执行；原目录已归档，不再覆盖）
python3 docs/research/experiments/2026-10-26-stokes-sparse/run.py

# 优先复算已有记录
python3 docs/research/experiments/2026-10-26-stokes-sparse/analyze.py

# 如需新测量
python3 docs/research/experiments/2026-10-26-stokes-sparse/run.py --out /tmp/blood-stokes-sparse-new
python3 docs/research/experiments/2026-10-26-stokes-sparse/analyze.py --results /tmp/blood-stokes-sparse-new

# 独立容差短测；后续复测同样应使用 --out 指定新目录
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-26-stokes-sparse/tolerance.py
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-26-stokes-sparse/analyze_tolerance.py

# 用户要求的 120 秒对照；已归档，复测请用 --out 指定新目录
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-26-stokes-sparse/longrun.py
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-26-stokes-sparse/replay_longrun.py
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-26-stokes-sparse/analyze_longrun.py

rustfmt --edition 2024 --check docs/research/experiments/2026-10-26-stokes-sparse/sparse.rs docs/research/experiments/2026-10-26-stokes-sparse/compare.rs
```

构建使用既有 Rapier 0.36.1、Rayon 1.12.0，`--release --offline --locked`，复用临时 target 缓存；没有下载求解库。每种实现实际执行：

```text
cargo test --release --offline --locked --manifest-path <临时副本>/Cargo.toml -p simulation --target-dir /tmp/sunyunbo/blood-stokes-rayon-target --lib fluid:: -- --nocapture
cargo build --release --offline --locked --manifest-path <临时副本>/Cargo.toml -p simulation --target-dir /tmp/sunyunbo/blood-stokes-rayon-target --example circulation_probe
flock /tmp/blood-stokes-feedback-bench.lock taskset -c 2-5 <候选二进制> 72 1 local <记录目录>
```

各四个检查通过，六条轨迹正常退出，复算及数值比较通过。没有运行全项目／Godot 检查或 UI 交互；本轮只增加研究文件与文档，正式源码未变。

## 测量口径

CPU 2–5、四线程、共用文件锁。72 细胞、64² 固定真实分叉，窗口内实际 3–5 细胞，另含机器人。每条 60 帧、一模拟秒、两个机械子步，不剔除初始帧。主性能不开启 `SPARSE_PROFILE`；独立 profiling 的 `PROFILE` 行覆盖一次背景求解及 120 次动态求解，分析时剔除背景求解。

`frames.csv` 的耗时来自生产步进／流体内部计时，不含快照、CSV 格式化、文件落盘或绘制。轨迹记录在步进计时之外，但可能影响后续缓存和调度，因此只与相同记录条件的匹配基线比较。p95 使用 nearest rank，尖峰不剔除。帧残差来自每帧最后子步，不能当作所有子步的最大值。

因子内存是持久数组有效载荷，未统计 Vec 头、容量余量、构造临时数据和进程峰值。`FACTOR` 行最后一项是单分量排序及分解时间；两个分量并行，不应相加当作初始化墙钟。

结果与限制见[报告](../../2026-10-26-stokes-sparse.md)。

追加容差组的流量记录覆盖每个子步；帧残差仍来自最后子步。流量收支按所有开口的面速度乘网格边长独立求和，比较的是总流量，不是各分支比例。宽容差运行没有执行或修改原严格测试；误差是否适合游戏尚需按实际场景选择。

## 120 秒结果与口径

`results/longrun/` 保存各 7,200 帧的 `loose` / `strict` 两条轨迹，以及 `replay-strict/` 的 1,800 帧同参数重放。原始帧诊断覆盖两子步的峰值及积分，不沿用旧短测仅最后子步的残差口径。`cells.csv` 包含初始、首帧和每秒样本，细胞穿越事件逐帧记录；大规模 ID 轨迹差异与总体通行表现分开分析。

长时探针只在临时复制的 `FluidStats` 内增加诊断值，未增加生产状态机制。速度／力有限值和原机械阈值检查通过；`1e-4` 的压力残差仍不符合原严格数值阈值。新增诊断有少量成本，长时耗时只在本组匹配探针之间比较。连续性缺口积分没有写回任何密度或流体体积状态。

上／下支路通量将左边界视作入口，其余开口按 y 正负分组，覆盖右边与上／下边相接的出口；该分组只用于当前固定地图。相对镜头的像素指标用机器人位置作目标镜头偏移，没有模拟 Godot 相机平滑或真实显示。

两分钟内候选稳定，局部累计流入量与严格对照相差约 0.052%，细胞平均累计路程相差约 0.70%，分叉通行总数均为 60；个体轨迹明显分岔。详细数值和游戏判断见[报告](../../2026-10-26-stokes-sparse.md)。
