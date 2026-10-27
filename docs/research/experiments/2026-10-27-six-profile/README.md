# 六次采样候选的进一步 profiling

2026-10-27。用户要求继续定位热点。本目录复用前一轮[六次采样实验](../2026-10-26-runtime-hotpaths/README.md)和更早的冻结 workspace，临时增加诊断，不改变数值算法或正式源码。

## 文件

- `run.py`：构造六次采样临时副本，分别运行控制、几何、流体、Rapier 原生计时及第二条控制轨迹。
- `profile.rs`：单个顺序 Simulation 的可选计数器；两速度分量任务可以并行记时，完成 join 后才读出。
- `analyze.py`：核对来源和观测一致性，复算阶段、调用数和嵌套／并行指标。
- `results/`：命令、构建日志、Cargo metadata、诊断锁、帧 CSV、快照指纹和统计。

无新增正式模块、机制或永久测试；不删除或移动前置实验源码。

## 实际运行与复现

仓库根目录执行。已有 manifest 时测量脚本拒绝覆盖；优先复算，新测量使用新目录。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-27-six-profile/run.py
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-27-six-profile/analyze.py

PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-27-six-profile/run.py --out /tmp/blood-six-profile-new
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-27-six-profile/analyze.py --results /tmp/blood-six-profile-new

rustfmt --edition 2024 --check docs/research/experiments/2026-10-27-six-profile/profile.rs
```

普通和 Rapier 原生计时构建均为 `cargo build --release --offline --locked ... --example runtime_probe`。原生计时额外指定 `--features rapier2d/profiler`；先离线生成该临时 workspace 的功能解析记录，既有包版本不得改变，诊断锁另存为 `Cargo-profiler.lock`，正式 Cargo 文件不动。功能解析细节由 `metadata.log` 保存，不只依赖 lock 包集合差异字段判断 feature 的可选依赖。

各轨迹实际命令形如：

```text
RUNTIME_PROFILE=1 DEEP_MODE=<none|geometry|fluid|rapier> \
flock /tmp/blood-stokes-feedback-bench.lock taskset -c 2-5 <诊断二进制> 30 <输出目录>
```

原始完整命令和退出码在 `results/manifest.json`。CPU 2–5、四线程、默认无操作、固定分叉、72 细胞、六次解析采样、四内部阻力点、`1e-4`、两机械子步；每条 1,800 帧，不去热身，不剔尖峰。

`perf stat -e task-clock true` 被系统拒绝（`perf_event_paranoid=4`）；未修改机器设置。没有获得 CPU 火焰图或硬件 cache/branch 事件，本轮结论来自墙钟计时和计数器。

## 读数注意

- `deep.csv` 按帧保存纳秒与调用数，`stages.csv` 保存原外层计时。快照、探针检查及 CSV 输出发生在模拟计时之外。
- 深层计数只在 `step_inner` 开始至结束之间有效；不包含初始化和快照几何查询。`none` 控制轨迹应全为零。
- 几何、流体、Rapier 是三个不同的诊断轨迹，不能相加成一帧。
- Schur、网格求解、速度逆与残差函数存在嵌套；路口混合也包含投影和管长，不能重复累加。
- `factor_*_work` 为两个分量累计的任务耗时，可能与帧墙钟重叠。逐次取分量耗时最大值与整个 inverse 的差，还包含等待和并行不足，不能都称为调度开销。
- 原生 Rapier 部分子计时可能没有覆盖某些并行路径；零计时不是删除该计算的证据。
- 几何深层埋点对总时间有可见影响。前后控制仅帮助观察扰动和波动，不支持从单轮时间推断准确的优化收益。

五条轨迹均复现已归档六次采样的快照指纹，并保持每帧非计时观测、每秒采样和通行事件一致；原机械范围检查保留。没有再次跑两分钟验收或 Godot/UI。

结果及待选择方向见[报告](../../2026-10-27-six-profile.md)。
