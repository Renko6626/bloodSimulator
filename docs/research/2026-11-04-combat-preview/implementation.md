# Task 1 实施报告

日期：2026-11-04。分支：`feat/immune-pathogen-combat`。未提交、合并或推送。Task 1 的代码与技术检查已完成，独立审阅由主代理组织。

## 实际交付

- 新增 `EntityCore`，Cell 组合持有并通过 Deref 保持内部生命访问简洁，机器人也持有同一基础生命记录、保持死亡停场。新增显式 `Species` 与 `TargetRef`，Cell 使用 Passive/Immune/Bacterium 行为枚举；物种判断不依赖行为 Option。沿用现有公开 `WhiteCellOptions`，其仍控制原中性粒参数及共享呼叫规则；其他物种数值来自已批准表，无新依赖。
- 局部事实列表只含活动且存活目标。免疫优先可感知直接攻击者，其次当前可见合法目标，再选最近可见威胁；失联只调查最后位置，可被新威胁打断。细菌反击可见免疫攻击者，否则寻找红细胞，不攻击机器人。近身三方向仅查询固定墙体，无路径、全局感知或恢复队列；全阻挡时停止主动推进并逐渐转向。巨噬准备和持续接触期间主动推进减半。
- 玩家 J 沿用真实轮廓与墙体遮挡，先于生物攻击结算。生物软体接触使用实际形变轮廓的点/边接近和边相交；机器人接触沿用实际软体轮廓与机器人接触半径。生物先收集当帧仍存活者动作，再同批应用伤害，支持互杀。批次后从仍存活攻击者按最大原伤害、同额稳定ID选择反击来源。荚膜只将免疫接触伤害乘0.5，机器人机械攻击不受影响。
- 只有玩家对正常/免疫细胞实际伤害计入通缉。保留50阈值、6秒安静后每秒下降2、呼叫1秒/共享8秒/总免疫上限8；上限包含休眠和尸体。预览80以上且无任何巨噬记录时优先尝试重型增援，巨噬最多1、失败消耗机会且不降级补兵。普通世界仍仅增援中性粒。
- 新物种 Hibernate，恢复本种半径及形态、生命和身份，清空接触准备；红细胞 Aggregate 和库存规则继续独立。所有尸体复用既有变软、萎缩及6秒清理。
- `Simulation::new_combat_preview` 与 Godot `--combat-preview`：原 small_world/解析流/Full，72红、3中性粒、1巨噬、4游动及2荚膜。按现有管腔合法贴壁空间确定坐标；近分叉与上下两支设置遭遇，巨噬在上支，另有下支可绕行。机器人出生留有距离。reset 重建同配置；普通入口新物种不投放。与organ/heart/LBM等模式冲突时明确退出。
- 快照 cells/cell_status 仍同序，桥迁移物种、目标位置/是否玩家、准备进度，删除旧 white/pursuing 显示字段。Godot 物种色彩、内部核图形、荚膜外层轮廓和朝向目标侧的准备弧已接入；玩家目标用红色提示。HUD 活动身体计数包含免疫/病原体，并显示 Immune wanted。
- README 写明启动方式，STATUS 与计划写入本轮范围、证据及限制。

## 新增永久测试

复用 runtime/immunity_tests.rs 的真实模拟夹具，新增6个场景，无数值表/文案测试：

1. 中立玩家附近发生免疫与细菌真实生命减少，通缉保持0，机器人未受伤、身体坐标有限。
2. 荚膜免疫接触伤害为4、玩家伤害50且不计通缉；同一玩家逻辑打红细胞计分。
3. 目标休眠失联后保留最后位置但清空准备，新可见目标不能继承接触准备。
4. 生物同帧互杀；玩家先击杀病原体后取消病原体反击。旧玩家先击杀免疫取消反击测试保留。
5. 巨噬/两种细菌真实休眠与恢复保持身份、生命及本种软体面积，准备清零；Local红库存始终0，病原体尸体不进入库存。
6. 预览数量、实际身体合法/有限、120帧推进与reset同一配置；巨噬休眠尸体仍占重型槽，移除后才可增援，普通世界不增援新种群。

## 验证命令与实际结果

| 命令 | 实际结果 |
| --- | --- |
| `cargo check --locked -p simulation --all-targets` | 退出0；迁移阶段的 Parry ShapeIntersection/ShapeCastOptions API 遗漏已修正 |
| `cargo fmt --all --check` | 退出0 |
| `cargo test --locked -p simulation` | 退出0；57单元＋5集成通过，0失败；Doc-tests 0项 |
| `cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu` | 退出0；包含例子、桥与GPU feature全目标 |
| `bash scripts/build.sh` | 退出0，桥构建完成 |
| `cargo test --locked -p simulation runtime::immunity_tests` | 末尾全受阻转向与休眠巨噬尸体断言调整后，受影响11项全部通过；其余46单元/5集成按范围不重复 |
| `godot --headless --path godot --editor --import --quit` | 退出0，无134，无脚本错误 |
| `godot --headless --path godot --fixed-fps 60 --quit-after 180` | world退出0，无脚本错误 |
| `godot --headless --path godot --fixed-fps 60 res://lab.tscn --quit-after 180` | lab退出0，无脚本错误 |
| `godot --headless --path godot --fixed-fps 60 --quit-after 180 -- --combat-preview` | 战斗预览退出0，无脚本错误 |
| `godot --headless --path godot --quit-after 5 -- --combat-preview --organ-preview` | 按预期退出1，明确报错 choose one world mode |
| `git diff --check` | 退出0 |

首次全套模拟测试56通过/1失败，失败是新增休眠测试误以为 streamed world 无人口池；该入口本来就有Local库存。修改测试明确启用Local并断言库存为0，未改生产行为或放宽预期。全套复跑57＋5通过。第一次feature检查还发现旧例子white字段及桥nullable Vector2转换遗漏，均已修正；最终全目标检查通过。

## 未验收项与范围

当前会话 DISPLAY 未设置，`/usr/bin/Xvfb`、`/usr/bin/xvfb-run` 不存在，没有完成渲染截图。Godot检查限定为加载和有限帧推进，不代表视觉人工确认、键鼠操作手感或战斗负载FPS验收。未运行自动点击/按键、GPU长回放、流场烘焙、SHA256、云实例、性能研究或额外全量脚本。未新增架构边界、依赖、导航、化学场、繁殖/感染模型或生产新种群。

## 文件清单

新增产品文件（untracked，审阅需显式纳入）：

- `crates/simulation/src/entity.rs`
- `crates/simulation/src/pathogen.rs`

修改：

- `crates/simulation/src/cell.rs`, `combat.rs`, `lib.rs`, `runtime.rs`, `view.rs`, `white_cell.rs`, `lbm_validation.rs`
- `crates/simulation/src/runtime/immunity.rs`, `gameplay.rs`, `streaming.rs`, `immunity_tests.rs`
- `crates/simulation/src/runtime/geometry_preview.rs`, `policy.rs`：旧快照计数字段必要迁移
- `crates/simulation/examples/heart_preview_probe.rs`, `organ_preview_probe.rs`, `organ_background_probe.rs`, `lbm_gameplay_probe/policy.rs`：旧字段必要迁移
- `crates/godot_bridge/src/lib.rs`
- `godot/world.gd`, `godot/world_hud.gd`
- `README.md`, `docs/STATUS.md`
- `docs/superpowers/plans/2026-11-04-immune-pathogen-combat.md`：仅Step完成勾选与实际证据；调度及审阅项由主代理维护

设计和计划原为主代理提供的未提交文件；设计文档未由实施者改写。报告位于 `.superpowers/sdd/2026-11-04-immune-pathogen-combat/task-1-report.md`。

## 独立审阅 Important 1 局部修正

完整审阅结论为 Critical 0 / Important 1 / Minor 0，报告 `review-report.md`。已核对根因：旧 `attacker` 在换到病原体目标后残留，而新目标持续刷新共用 memory，使早先的 Robot 身份可无限保留。细菌主动放弃全部目标时也有同类清理缺口。

修正只涉及 `white_cell.rs::set_target`：当新目标不等于已记录直接攻击者时清空 `attacker`。直接受击事件仍能保留与新目标一致的来源；换目标或清空目标会放弃旧证据，沿用现有目标/记忆字段，没有新增仇恨计时、队列或接口。

新增一个合并真实回归 `abandoned_attackers_do_not_override_current_or_neutral_targets`（本阶段永久新增共7场景）：玩家实际伤害中性粒50点，离开12秒，中性粒切到持续可见荚膜细菌，通缉归0，玩家回来仍应继续病原体目标；另验证荚膜受到免疫实际伤害后，来源休眠造成无目标时清空来源，免疫恢复也不凭旧证据自动反击。

- 修正前：`cargo test --locked -p simulation abandoned_robot_attacker_does_not_interrupt_a_current_pathogen_target` 退出101，实际 `Some(Robot)`，预期 `Some(Cell(12))`，明确复现独立审阅问题。
- 修正后：`cargo test --locked -p simulation runtime::immunity_tests` 退出0，12项通过；其余46单元和5集成按范围不重复。
- `cargo fmt --all --check`、`git diff --check` 退出0。测试编译同时验证本地实现；没有接口/Godot变更，未重跑全量feature或Godot。
- 修正前原文件：`white_cell.before-review-fix.rs`、`immunity_tests.before-review-fix.rs`。
- 修正前相对HEAD diff：`pre-review-fix.diff`。
- 仅本次修正与新增回归的前后diff：`review-fix.diff`，用于 scoped re-review。

上述文件均位于 `.superpowers/sdd/2026-11-04-immune-pathogen-combat/`。没有额外发现或扩展机制，仍未提交/推送。

- 修正后补跑 `bash scripts/build.sh`：退出0（3.00秒），本地Godot桥已包含attacker清理修正。没有重复场景加载或全量检查。
