# 免疫与病原体战斗实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. 本次用户明确指定 Sol 子代理实施与审阅，按下列步骤连续执行。

**Goal:** 在现有模拟中实现共用实体基础、两种免疫细胞、两种细菌及独立战斗预览入口。

**Architecture:** Rust 继续持有权威状态，Rapier 保持同一物理世界，Godot 接收输入并显示快照。共用 EntityCore、Species、TargetRef，物种行为采用小型枚举，战斗通过当帧临时列表结算。所有细节遵循已批准设计。

**Tech Stack:** 现有 Rust、Rapier、Godot；无新增依赖。

**Spec:** `docs/superpowers/specs/2026-11-04-immune-pathogen-combat-design.md`

## Global Constraints

- 用户 2026-11-04 已要求先 plan 后派发 Sol 实施、审阅，授权连续执行，无需重复确认计划。
- 工作目录 `/data/sunyunbo/www/bloodSimulator`，功能分支 `feat/immune-pathogen-combat`；沿用当前构建缓存，不新建工作树或重建依赖环境。
- 不做 A*、化学场、全图导航、队形、复杂脱困；只做设计中三方向近身避墙。
- 不烘焙流场、不校验 SHA256、不研究性能、不启动云实例、不改 LBM 算法或子步。
- 第一批仅中性粒、巨噬、游动细菌、荚膜细菌；不做繁殖、感染链、组织预算或任务系统。
- 默认 UI 验证仅加载与截图，不自动点击或操作键鼠。
- 新永久测试仅覆盖新增关键行为，复用已有夹具；已有检查通过后结束，不做重复全量验证。
- 不提交或推送；保留完整可审阅工作树。子代理不再派生代理，主代理统一组织审阅。

## Review Focus

1. 新物种不能进入红细胞库存；恢复必须使用本种身体半径，恢复后接触准备清零。
2. 病原体及免疫细胞互战不能增加玩家通缉；机器人伤害病原体也不计分。
3. 目标死亡、休眠或遮挡后，不能依赖实时坐标打击；换目标重新准备。
4. 玩家当帧击杀取消反击；生物接触攻击同批结算支持互杀，不依赖实体遍历顺序。
5. 普通世界保持四只中性粒，重型增援和新种群仅预览启用；reset 保持入口及配置。

## 执行结构

实体分类、目标选择、伤害归属、快照与入口相互耦合，作为一个可验收功能交给一位 Sol 实施者连续完成，避免多人同时迁移白细胞字段。下列步骤是内部交付顺序。完成后由另一位 Sol 独立审阅规格符合性和代码质量，必要修正仅复测受影响检查。

### Task 1: 实体与三方战斗完整接入

**Files / ownership:** 实施者负责 `crates/simulation/src/{entity.rs,cell.rs,white_cell.rs,pathogen.rs,combat.rs,config.rs,view.rs,lib.rs,runtime.rs}`、`runtime/{immunity.rs,gameplay.rs,streaming.rs,immunity_tests.rs}` 及需要的小型新行为/测试模块；`crates/godot_bridge/src/lib.rs`、`godot/{world.gd,world_hud.gd}`，必要时 `world_cell_mesh.gd`；README、docs/STATUS.md 与本计划的实际证据。若旧例子和测试直接读旧字段，仅做编译所需迁移。主代理拥有设计文档与计划调度段落，实施者不撤销其他改动。

**Interfaces:**
- `EntityCore { id: u64, health: f32, max_health: f32, dead_age: Option<f32> }`，共用 `damage(amount: f32)`；机器人通过它保持原有停场语义。
- `Species::{RedCell, Neutrophil, Macrophage, MotileBacterium, EncapsulatedBacterium}`，明确 `is_immune()`、`is_pathogen()`；不以行为 Option 是否存在推断物种。
- `TargetRef::{Robot, Cell(u64)}`；具体行为枚举命名、内部辅助函数由实施者按现有代码决定，避免重复保存可推导状态。
- `Simulation::new_combat_preview(config: Config) -> Self`；场景初始化明确记录 preview 模式以供 reset；普通世界仍 `new_world`。
- 快照保持 `cells` 与 `cell_status` 同序，状态增加物种、动作目标位置/是否针对玩家及必要准备进度；桥与 Godot 同步迁移，不维护无消费者的冗余旧字段。

- [x] **Step 1: 建立基础记录与物种配置，迁移现有调用。** 新建 entity 模块，将 Cell 和机器人生命迁入共用基础，替换红白二分；保持红细胞聚合、休眠、尸体、旧探针语义。物种半径、生命及其他数值逐项取设计表；十二节点不变。已有 WhiteCellOptions 的公开使用者只做必要兼容迁移，不扩展配置框架。运行 `cargo check --locked -p simulation --all-targets` 捕获迁移遗漏。

- [x] **Step 2: 接入局部行为和目标关系。** 免疫细胞读取可见病原体、可追捕机器人和局部受击证据；直接攻击者优先，其次有效旧目标，其次最近合法目标。失去感知只使用最后位置，遇新可见威胁可打断调查。细菌优先反击可见的免疫攻击者，否则损伤正常红细胞，不攻击机器人。采用既有 heading/memory/cooldown，无重复仇恨列表。近身三方向查询只检查固定墙体，不做长路径。巨噬准备和连续接触期间主动速度减半。

- [x] **Step 3: 统一命中与伤害归属。** 保留机器人 J 的实际轮廓命中与遮挡。生物对软体的接触判断必须使用真实形变轮廓的相交/足够近的接触，不能只用中心距离或理想半径：优先复用 Rapier 几何查询或现有 contour helper，小规模逐对扫描足够。生物对机器人沿用实际轮廓与机器人半径。玩家攻击先结算，其后收集活生物接触动作并同批应用；荚膜仅将免疫攻击伤害乘 0.5。有效伤害决定通缉，来源不可混淆。无事件总线、空间索引或异步机制。

- [x] **Step 4: 扩展通缉与增援。** 保留计分、阈值 50、6 秒安静后每秒下降 2、共享呼叫 1/8 秒和免疫总上限 8。预览通缉 80 以上尝试巨噬，总巨噬最多 1 且包括休眠/尸体；失败跳过且消耗机会。普通世界仅中性粒。合法贴壁生成按物种尺寸，入场 0.8 秒；不增加恢复队列。休眠恢复清空接触准备，其他计时遵循设计。

- [x] **Step 5: 接入预览与显示。** `--combat-preview` 使用 small_world、解析流、Full，约 72 红细胞，3 中性粒、1 巨噬、4 游动与2荚膜；精确坐标由合法空间决定，形成局部交战与可绕行巨噬遭遇。与 organ/heart/LBM 入口互斥，冲突给明确错误。普通入口不加病原体。桥/显示区分物种、动作准备和玩家目标，荚膜外轮廓；HUD 计数是活动身体，展示“Immune wanted”或符合既有界面语言的自然文案。reset 恢复本配置。

- [x] **Step 6: 只补关键真实模拟测试。** 复用 `runtime/immunity_tests.rs` 等夹具，建议最多约 6～8 个新增场景，可合并紧密相关断言；不为数值表/文案写测试。必须覆盖：中立玩家旁免疫与细菌发生真实生命减少且通缉为零；机器人打病原体不加通缉但打正常细胞有计分；目标丢失/换目标不能继承接触准备；荚膜对免疫与玩家攻击结果不同；生物互杀与玩家击杀取消反击；新类型休眠恢复及红库存隔离；预览 reset 与重型增援限制。尽量扩展已有有效测试，独立预期值，不复制算法。不做 UI 自动交互。

- [x] **Step 7: 集中执行相关检查并修复。** `cargo fmt --all --check`、`cargo test --locked -p simulation`、`cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`。运行 `bash scripts/build.sh` 构建桥，Godot cached import 和普通 world/lab/combat-preview 有限帧 headless 加载（约 120～180 帧）。如有已知首次冷 import 134，仅记录原结果并按已有做法缓存重试，不扩展根因研究。必要截图一张验证物种和 HUD，不做键鼠动作；截图环境受限则报告，不假称验收手感。不再运行 scripts/check.sh 重复同一整套检查，也不运行 GPU 长回放。

- [x] **Step 8: 记录实际交付。** README 写预览启动方法；STATUS 增加本阶段行为、命令、实际结果和未验收手感/FPS。更新本计划完成勾选及证据。实现报告写到主代理提供的临时报告路径，记录实际改动、测试命令/结果、偏离与剩余问题，不能将“编译通过”写成手感验收。


### Task 1 实施证据（2026-11-04）

- `cargo check --locked -p simulation --all-targets` 通过；`cargo fmt --all --check` 通过。
- `cargo test --locked -p simulation`：57 单元、5 集成通过。新增6个关键场景；第一次新休眠测试错误假定 streamed world 没有红库存，修正为 Local 实际库存始终0后通过，未修改生产断言或放宽预期。
- `cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu` 通过。迁移中修正旧 feature 例子字段及桥的 nullable Vector2 表达。
- `bash scripts/build.sh` 通过。末尾局部全受阻转向与休眠巨噬尸体断言调整后，`cargo test --locked -p simulation runtime::immunity_tests`：11项通过。
- `godot --headless --path godot --editor --import --quit`：退出0；普通 world、`res://lab.tscn`、`--combat-preview` 均以 `--fixed-fps 60 --quit-after 180` 加载，退出0、无脚本错误。
- `godot --headless --path godot --quit-after 5 -- --combat-preview --organ-preview`：按预期明确报错并退出1。
- 后续按用户要求截图：发现已有 X 显示服务 :2，通过软件 OpenGL 完成真实预览初始全局与上方血管近景，见 `docs/research/2026-11-04-combat-preview/`。无自动键鼠，人工手感与战斗负载 FPS 未验收。
- 独立审阅 Important 1 局部修正：目标切换/放弃时清理旧 attacker，无新计时系统。真实离场12秒回归修正前失败、修正后 immunity 12项及fmt/diff检查通过，`bash scripts/build.sh`重新构建桥通过；新增永久场景共7个。无接口改动，未重复Godot或全量检查。定点diff见 `.superpowers/sdd/2026-11-04-immune-pathogen-combat/review-fix.diff`。
- 新产品文件：`crates/simulation/src/entity.rs`、`crates/simulation/src/pathogen.rs`；详细实现与命令报告：`.superpowers/sdd/2026-11-04-immune-pathogen-combat/task-1-report.md`。

## 审阅与收尾

- [x] 独立 Sol 阅读设计、计划和实际 diff，分别给规格符合性与代码质量结论；聚焦核心行为和授权范围，不因理论边角问题新增机制。
- [x] 必要修正由原实施者完成；复测影响范围，审阅者只复查修改点，不重启全部验证。
- [x] 主代理核对产物与实际命令结果，报告预览入口、完成范围、未验收项；不自动合并或推送。

## 调度记录

- 起点：`4ff964b`，现有设计文档为本次未提交文件；当前分支 `feat/immune-pathogen-combat`。
- Ruling：用户已给定执行方法与“先 plan 再执行”的授权，连续推进，不追加计划确认轮。
- Ruling：沿用当前目录及构建缓存，通过功能分支承载改动；不为流程额外建立工作树或跑一轮重复 baseline。
- Preflight：Task 1 各步骤顺序消费同一实体/目标接口，单一实施者拥有所有产品文件；内部步骤与 Global Constraints 一致。审阅只读且在实施完成后启动，无并发产品写入。

## 独立审阅结论

Sol 完整审阅发现一项核心问题：换目标后旧攻击者标记残留，低通缉玩家返回仍会被优先追捕。已局部修正目标清理并补真实回归，先复现失败后相关12项通过；Sol 仅复查修正点，最终规格符合性及代码质量均通过，无剩余发现。静态截图已随后完成；人工手感与新战斗负载FPS仍未验收。

## 阶段集成

用户已要求整理提交、合并 main 并推送。本次保存产品代码、设计、计划、实现/独立审阅报告及截图；沿用已有模拟与引擎验证，只复核格式和 feature 全目标编译，不追加流场烘焙、长回放或云实验。
