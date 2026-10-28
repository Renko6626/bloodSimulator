# LBM 可玩验证场只读审阅

对象：`feat/lbm-gameplay-validation`，基线 `14bf6d3`，工作树 `/data/sunyunbo/.codex/worktrees/8d56/bloodSimulator`。包含 untracked `lbm_validation.rs`、`lbm_gameplay_probe.rs`、本轮计划与分析脚本。没有修改工程文件、运行 Cargo、启动实例或派生代理。

## 定点复核结论

**Ready：三项重要问题和压力标签 minor 均已修正，可以冻结产物并进行已批准的 GPU smoke / 固定与移动实测。** 本次仅重新读取对应修正位置，没有运行 Cargo 或开展全面重审。远端 Godot 启动结果、实卡稳定性、实际接触负载和性能仍须由计划中的实测给出；ready 不意味着这些已经通过。

- 外侧 `(3,4)` 已改成 `(-14,100) → (60,100) → (100,140)`，与 `(4,5)` 仅在公共端点连接，不再反向重合；原窗口内 fork 和两 route origins 不变。主代理报告重算压力为 12.605889、分流为 10.55135 / 7.9779463，后续以正式 status 中实际 flow 为准。
- probe 在 snapshot 缺失时优先附上 try_tick 的原始 Err；收尾使用最后有效的 region_switches，不再 unwrap 缺失 LBM。主代理提供临时 fault 注入证据：frame 1 Err + snapshot None 时 exit 1，status 保留原始错误，无 panic。本审阅核对了对应代码，没有重复故障试验。
- render 分析通过配套主 CSV 按 physics_frame−1 映射，要求间隔前后两端均属于 measure 且 interval>0；active/contact 等分组同时取对应物理记录。主代理提供临时四行夹具证据：排除首行零间隔和 warmup 边界，余两行均值 15 ms。本审阅核对了筛选代码及 `.csv.render.csv` 对应主 `.csv` 的路径。
- HUD 通过 set_pressure_display 同步 slider 与 label，并保留无参数信号的初始化行为。

## 初次审阅记录（以下三项现已修正）

**初审要求先完成下面三项局部修正，再冻结正式实验产物；现已完成，不需要新增架构或扩大实验矩阵。** 未发现本轮改变原 LBM 数值算法、默认 small_world 或正常机械推进的情况。问题已即时发给主代理，probe 修正已交实施者。

### 重要 1：外侧回路两段反向重合

位置：`crates/simulation/src/lbm_validation.rs:25`，连同节点及 `(4,5)` 默认直线。

`(3,4)` 经 `(-14,140)` 到 `(100,140)`，而 `(4,5)` 从 `(100,140)` 返回 `(-50,140)`；两条图边在 114 世界单位长度上完全反向重合。GraphNetwork 把它们视为独立阻力管，实际地图使用管道并集，窗口外解析速度又按 nearest-edge 选择边，重合位置无法同时具有两条相反管流。玩家或细胞进入此段时，实际几何连接/流向与闭合图模型不一致。虽然在当前两个局部窗口外，它属于本轮新增正常可玩场景的真实地图错误，且没有 respawn 保证永久不进入。

建议只调整外侧返回路径以避免重合，保留窗口内 fork 及 route origins，并重新记录图求得的流量；无需更换算法或扩大地图能力。

### 重要 2：失败收尾再次 panic，覆盖原始原因

位置：`crates/simulation/examples/lbm_gameplay_probe.rs:163`；相关 `:116` 在检查 `step` 原始错误前要求 `snapshot.lbm` 存在。

当最后 batch/finish_frame 返回错误时，pending cache 会使 `snapshot.lbm=None`、`lbm_status=...`。循环先返回 missing LBM，收尾又调用 `sim.snapshot().lbm.unwrap()`，随后 panic。因此原始 step 错误及 completed/result 状态行丢失；已写入 CSV 的 prefix 会 flush，但失败来源和结束状态不完整。这是本轮明确要求保留失败证据的路径，不是要求新增恢复。

建议优先保留 step 原始错误，收尾安全读取 Option 或记录最后有效快照，不改变失败即退出的行为。

### 重要 3：渲染统计把预热当测量

位置：`docs/research/2026-10-29-lbm-gameplay/analyze.py:30`、`:79`，对应 `godot/world.gd:170`。

主 CSV 有 phase，render CSV 没有；分析器遍历全部 CSV 并对缺失 phase 默认 measure，因此 render 的 all_measured 会包含预热、截图影响和首次零间隔行。与本轮“预热 2 秒，测量 3600 帧”的统计边界不一致。

利用已有 physics_frame 和对应主 CSV 的测量范围筛选渲染行即可；渲染间隔仍须称为实际显示循环间隔，不能当纯 GPU render duration。

## 已核对且未发现重要问题

- 配置通过既有 GraphNetwork 求 unit flow，再标定入口 Q；输出实际 branch_flows，没有把旧 70:30 当成本轮结果。
- 72 个十二节点实体使用原材料、拖曳和 Rapier 插入；seed 有真实管壁/实体间距限制。reset 经 validation_scene 回到同一地图和种子，保留当前 pressure、paused 与所选 profile。
- 正常 `--local-lbm --lbm-validation` 入口复用桥接，非 benchmark 仍读 WASD。Rust 与 Godot 回放均通过位置误差生成 MoveInput，12 秒一段、目标 x10/x28，不直接改机器人或粒子位置。
- probe 预检初始与另一个 origin，实际 region_switches/robot 坐标被记录，长移动案例无迁移会失败。active_cells 来自实际 marker 耦合，nearby/soft_contacts 是独立观测，没有用 cell_count 代替局部负载。
- Rust `step_wall_ms` 包围 try_tick，snapshot 与 quality 单独计时；Godot tick 与 snapshot_refresh 单列，后者包含 bridge quality/HUD 更新。CSV 开销不计入 simulation wall，但会进入实际 process interval。报告应保留此边界。
- 失败数值、几何不支持、守恒残差与机械 guard 有明确退出；未放宽 LBM 几何或添加静默回退。

## Minor、验证与范围

验证场初始化用 `set_value_no_signal` 更新压力滑块，但既有 label 通过 value_changed 更新，初始文字仍可能显示 12.0；不影响模拟或 benchmark。可在收尾局部同步文本。

只读执行 `git diff --check` 返回 0，并检查源码与计划。没有重复本机 Cargo 或 GPU 检查。主代理提供的既有证据为 CPU seed/reset、软件 2 帧、release build 与 Godot check-only 通过；并行重链接期间 editor import exit134 仍需真实远端启动验证，不能由静态审阅认定已排除或直接归因代码。

未要求全游戏验收、完整角度/分辨率矩阵、换耦合算法、自动故障恢复、GPU reduction 或新永久测试工程。正式实卡结果出来前不判断性能达标、60 秒数值稳定或接触负载是否充足。
