# 单次几何查询的匹配验证

2026-10-27。用户批准最近管段与带符号距离复用。本实验使用已接受的六次解析采样 + 稀疏 Stokes `1e-4` 候选，唯一差异是两个正式共享源码文件中的等价几何优化。

- `run.py`：从上一轮冻结输入构建优化前和候选；候选直接复制当前生产 `map.rs` / `circulation/graph.rs`，所有其他模拟逻辑保持冻结版本。
- `audit.rs`：只注入临时单元测试构建，使用归档旧速度采样作为对照，在实际地图范围、零／正／负压力下比较 54,495 次查询，要求 f32 逐位相同。
- `analyze.py`：核对来源哈希、完整轨迹、非计时诊断、快照指纹、采样和通行事件；复算阶段及全图查询调用数。
- `results/`：原始命令、退出码、二进制／来源哈希、静态审计日志、帧 CSV 和复算统计。

不修改或覆盖前置实验源码。原最近管段 API 保留，新增合并方法为 crate 内部接口；不引入缓存字段或依赖。

## 实际执行与复现

仓库根目录运行。默认结果已归档时不能覆盖；新测量请指定新目录。

```bash
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-27-geometry-query/run.py
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-27-geometry-query/analyze.py

PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-27-geometry-query/run.py --out /tmp/blood-geometry-query-new
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-27-geometry-query/analyze.py --results /tmp/blood-geometry-query-new

bash scripts/check.sh
rustfmt --edition 2024 --check docs/research/experiments/2026-10-27-geometry-query/audit.rs
```

临时工程的静态审计与构建采用：

```text
cargo test --release --offline --locked --manifest-path <临时工程>/Cargo.toml -p simulation --target-dir /tmp/sunyunbo/blood-stokes-rayon-target --lib geometry_query_audit -- --nocapture
cargo build --release --offline --locked --manifest-path <临时工程>/Cargo.toml -p simulation --target-dir /tmp/sunyunbo/blood-stokes-rayon-target --example runtime_probe
RUNTIME_PROFILE=1 DEEP_MODE=<none|geometry> flock /tmp/blood-stokes-feedback-bench.lock taskset -c 2-5 <二进制> 30 <输出目录>
```

72 细胞、12 个软体粒子、六次解析流速查询、四个内部流体标记、固定真实分叉、两个机械子步、默认无操作输入；每条 1,800 帧，不去热身。CPU 2–5、四线程、共享文件锁。主性能顺序为优化前／复用、复用／优化前，另各一条独立几何 profiling。

## 计时与数值口径

保留外层阶段计时，主性能关闭深层计时。独立几何轨迹在模拟步内启用计数，快照、绘制和 CSV 不计入步骤；旧 `nearest_edge` 和新合并方法均使用 `deep.csv` 的 `nearest_edge` 指标，新语义为“最近管段与距离合并查询”。候选解析采样内的 `signed_distance` 调用数应为零，独立 API 本身仍保留给其他用途。

所有配置的每帧 f32 顶点／机器人／箭头快照指纹、每帧非计时诊断、逐秒细胞／箭头采样、通行事件与优化前一致。指纹沿用前置探针的 FNV-1a 回归诊断，不作为完整内部状态或密码学证明。既有机械检查范围保留，宽容差流体不声称通过原严格残差界。

项目检查包含现有 Rust 测试、扩展构建和 Godot 两场景 headless 导入／短时启动，未自动输入或操作 UI。没有新增正式永久测试。

结果与限制见[报告](../../2026-10-27-geometry-query.md)。
