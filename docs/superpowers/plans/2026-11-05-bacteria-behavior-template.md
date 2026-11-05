# 基础细菌行为模板实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 细菌独立决策，主动寻找红细胞，按种类配置反击或逃离，允许反击机器人。

**Architecture:** `Behavior` 区分免疫与细菌决策，复用小型运动/接触攻击记录。运行期提供局部感知、伤害事实并执行统一的身体驱动与伤害结算；快照接口保持原状。

**Tech Stack:** 现有 Rust、Rapier、Godot；无新增依赖。

**Spec:** `docs/superpowers/specs/2026-11-05-bacteria-behavior-template-design.md`（用户于本会话明确“开工”，已确认）。

## Global Constraints

- 细菌感知半径 4、墙体遮挡、默认反击、默认逃离余时 0.8 秒；速度与攻击数值不变。
- 逃离期间无攻击目标，冷却正常推进；威胁不可感知后仅保存最后方向。
- 免疫行为、休眠/尸体/通缉、普通种群和 Godot bridge 字段保持既有契约。
- 不增加繁殖感染、新框架、寻路、UI配置、性能或云实验；不运行自动UI交互。
- 用户的检查范围与测试质量约束优先；集中验证，不按机械步骤反复运行全量检查。

## Review Focus

- 接触攻击不能把逃离威胁当作伤害目标；用真实接触与生命断言覆盖。
- 机器人受击来源能触发自卫，平时中立；失去感知后旧仇恨不会恢复。
- 隐藏威胁的移动不影响逃离方向；用墙体遮挡后的运动与余时覆盖。
- 免疫独立记忆、呼叫和入场状态不能被共用记录拆分破坏；沿用既有用例。
- reset 保留细菌配置，休眠冻结逃离计时并在恢复后清空接触准备；复用已有场景夹具覆盖。

### Task 1: 行为拆分与配置接线

**Files:** 新增 `crates/simulation/src/behavior.rs`；修改 `pathogen.rs`、`white_cell.rs`、`config.rs`、`lib.rs`、`cell.rs`、`runtime.rs`、`runtime/immunity.rs` 及现有 `runtime/immunity_tests.rs`、`runtime/streaming.rs` 的测试访问。

**Interfaces:** `ActiveBehavior` 保存 target、heading、movement、patrol_time、contact_time、attack_cooldown，并提供 `new(id)`、`set_target(Option<TargetRef>)`、`steer(direction, speed, dt, turn_time)`、`patrol(dt)`。`Behavior` 以 `active()/active_mut()` 读取共用记录，以 `immune()/immune_mut()` 读取免疫状态。`Bacterium` 拥有独立威胁/逃离状态及共用记录。`BacteriaOptions` 提供 `motile`、`encapsulated` 两个 `BacteriumOptions { response: ThreatResponse, flee_duration: f32 }`。

- [x] 在既有真实模拟夹具加入机器人受击反击用例，先观察旧代码失败：未受击时机器人不掉血，实际非致死攻击后细菌接触伤害有效。
- [x] 抽出共用记录与行为枚举，保留免疫决策；细菌以 `think(center, visible, dt, movement_options, response_options)` 接收可感知事实，以 `react_to_hit(source, position, center, options)` 接收直接受击。运行期分发两类决策，复用转向、接触与伤害。
- [x] 接入配置，默认两种细菌均反击，余时 0.8。反击机器人与免疫攻击者，恢复猎物选择与失去感知清除逻辑。
- [x] 更新旧测试的状态读取，保持有效行为断言。运行 `cargo test --locked -p simulation immunity_tests`，预期原有与新增反击用例通过。

### Task 2: 逃离与生命周期覆盖

**Files:** `pathogen.rs`、`runtime/immunity.rs`、`runtime.rs`、`runtime/immunity_tests.rs`。

**Interfaces:** 使用 Task 1 的配置与共用输出，逃离只持有威胁引用、最后方向和余时，`target_position` 对逃离返回 `None`。

- [x] 新增真实模拟用例覆盖：逃离运动且无伤害、遮挡后方向保持并到时恢复捕猎、切换清空准备且保留冷却、休眠冻结及reset保留配置。保留/扩展原有机器人中立、病原体互战和猎物损伤证据，避免重复测试同一失败风险。
- [x] 实现逃离规则，接线休眠恢复和快照；不将威胁引用输出为攻击目标。
- [x] 针对新增用例运行 `cargo test --locked -p simulation immunity_tests`，预期通过；失败先定位生产路径或夹具原因，不放宽业务断言。

### Task 3: 集中验证、审阅与交付记录

**Files:** 本计划、设计状态与 `docs/research/2026-11-05-bacteria-behavior/README.md`。

- [x] 执行 `bash scripts/check.sh`（fmt、workspace编译、simulation测试、桥构建、Godot导入/world/lab加载），集中记录输出。追加战斗预览短加载和 `cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`，不做GPU实验。
- [x] 一次独立只读整分支审阅，重点检查上述 Review Focus；仅处理实际业务缺陷，不扩张机制。
- [x] 如有修正，仅运行受影响检查。记录实际结果、启动方式与配置示例；人工手感及FPS保持未验收。
- [x] 本地提交功能与交付记录，保留分支/工作树，不自动推送或合并。

## 执行记录

- 工作区：宿主管理的 `fcc8/bloodSimulator`，分支 `feat/bacteria-behavior`，基础提交 `0bd496a`。
- 用户已要求开工，本会话直接执行；不再次请求计划或执行方式授权。

- Task 1/2：原有12项战斗检查通过；机器人反击新增用例先失败（100≠92），实现后通过。逃离两项新增用例及休眠速度断言先失败，接入后16项战斗检查通过。
- 实现细节：受击入口额外接受当前位置和来源是否可感知，保证反击不追隐藏来源、逃离可使用事件位置。按用户约定完整改动集中验证，不逐函数新增测试。

- Task 3：`bash scripts/check.sh`通过（64单元+5集成、桥构建、首次导入与world/lab加载）；feature全目标编译及10帧战斗预览通过。独立Astra代码审阅无待修复问题。详见[阶段记录](../../research/2026-11-05-bacteria-behavior/README.md)。本地保存并保留工作树，不推送或合并。
