# 运行时分段计时与零拖曳采样实验

2026-10-26。用户批准后续优化列表第 1 项。复用[稀疏候选实验](../2026-10-26-stokes-sparse/README.md)的冻结 workspace、稀疏 Cholesky、`1e-4` 容差和长期诊断，主工程保持原样。本目录不复制一套新的基线，不能删除其前置实验源码。

## 文件与范围

- `run.py`：构建两份临时工程；同样的可选阶段计时和调用计数，唯一候选差异为零拖曳时跳过解析采样，保留 Rapier setter。
- `probe.rs`：沿用上一轮的逐帧机械／流体诊断和每秒采样，另记录阶段 CSV 及每帧快照指纹。
- `analyze.py`：核对来源哈希、完整帧数、分段覆盖、实际调用数、非计时诊断和轨迹一致性，复算匹配性能。
- `six_sample.py` / `analyze_six.py`：按用户后续指示，将每细胞解析流速查询降至六次，给其他六个物理节点插值；独立与十二次采样比较耗时及误差。两组均保留零拖曳跳过。
- `results/`：命令、来源和二进制哈希、已有检查日志、每帧 CSV、采样和统计。

不修改求解频率、边界、反馈强度、空间索引、物理材料或任何依赖。临时增加的 `StageProfile` 仅服务本实验，不进入正式接口。

## 复现

仓库根目录执行：

```bash
# 实际执行的测量命令，默认结果现已归档，不再覆盖
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-26-runtime-hotpaths/run.py

# 优先复算保存的数据
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-26-runtime-hotpaths/analyze.py

# 用户追加的六次采样实验与复算；默认结果同样已归档
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-26-runtime-hotpaths/six_sample.py
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-26-runtime-hotpaths/analyze_six.py

# 需要新测量时指定新的输出目录
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-26-runtime-hotpaths/run.py --out /tmp/blood-runtime-hotpaths-new
PYTHONDONTWRITEBYTECODE=1 python3 docs/research/experiments/2026-10-26-runtime-hotpaths/analyze.py --results /tmp/blood-runtime-hotpaths-new

rustfmt --edition 2024 --check docs/research/experiments/2026-10-26-runtime-hotpaths/probe.rs
```

两种构建实际运行以下已有检查及构建：

```text
cargo test --release --offline --locked --manifest-path <临时副本>/Cargo.toml -p simulation --target-dir /tmp/sunyunbo/blood-stokes-rayon-target --lib cell::tests::compressed_cell_preserves_area_and_recovers_after_release
cargo build --release --offline --locked --manifest-path <临时副本>/Cargo.toml -p simulation --target-dir /tmp/sunyunbo/blood-stokes-rayon-target --example runtime_probe
flock /tmp/blood-stokes-feedback-bench.lock taskset -c 2-5 <二进制> 30 <输出目录>
```

## 测量口径

72 个 12 节点细胞、固定真实分叉、每帧两子步、默认无操作输入；每条 30 模拟秒、1,800 帧，不剔除热身帧。CPU 2–5、同插槽四物理核、四线程、release、offline、locked、共用研究锁。主性能顺序为基线／候选、候选／基线，另各运行一条独立 profiling 轨迹。计时结束后再采集快照、写 CSV，所有构建使用相同记录方式。

`RUNTIME_PROFILE=1` 才开启阶段计时与实际解析采样调用计数；正常主性能关闭。`stages.csv` 中六个阶段互不重叠，`whole_ms − 六阶段之和` 为未覆盖的调度余量。细胞解析驱动含局部权重、采样和速度设置，不能视作采样函数纯净耗时；流体阶段包装计时与其内部 `FluidStats.solve_ms` 有少量边界差别；Rapier 是步进墙钟成本。profiling 调用计数也有开销，不能直接作为主性能结果。

快照指纹以固定字节序对全部 f32 细胞顶点、机器人位置、箭头位置和速度做 FNV-1a；它是回归诊断，不是密码学或完整内部状态证明。独立比较逐秒采样 CSV、通行事件和每帧非计时数值。未使用源码字符串断言来替代数值行为验证。

机械范围沿用原实验：面积偏差小于 25%、越壁小于 0.15、交换冲量误差小于 `1e-7`。宽容差场不声称满足原严格压力残差界。没有运行 Godot、自动 UI 操作、全项目检查或再次扩展 120 秒长期验证。

结论见[研究报告](../../2026-10-26-runtime-hotpaths.md)。

## 六次采样追加组

结果单独保存在 `results/six-sample/`。新主性能基线为已有零拖曳跳过 + 十二次采样，候选为零拖曳跳过 + 六次采样；每种两条 30 秒轨迹，第二轮反序，再各一条独立 profiling。重测可给 `six_sample.py --out <新目录>`，复算相应用 `analyze_six.py --results <新目录>`。

每细胞仍为 12 个物理节点、局部 Stokes 仍为四内部阻力标记。候选取节点 0、2、4、6、8、10 的位置查询流速，奇数节点目标流速为两侧偶数节点查询值的平均，节点 11 用 10 与 0 插值。全部节点保留同一速度松弛系数。此实现仅覆盖固定 12 节点研究场景，未建立通用重采样框架。

追加组实际执行 `cargo test ... --lib cell::`，保留已有挤压恢复检查，并在临时研究构建加入一项均匀流全环响应检查。两种构建各两项通过；没有新增正式永久测试。`analyze_six.py` 对同配置重复运行要求观测记录一致，对十二／六次采样之间则记录近似误差，不声称物理等价。

解析驱动的 profiling 均值约 5.47→2.41 ms，主性能整步两轮约 17.61 / 14.34→11.41 / 10.89 ms，宏观路程差约 0.013%；共享主机波动、不同轨迹和各阶段相互影响均保留。候选最大尖峰约 306 ms，不保证稳定帧率；尚未继承前一轮两分钟验收。
