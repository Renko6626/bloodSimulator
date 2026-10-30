# 白细胞与免疫警觉度实施计划

> **For agentic workers:** Use superpowers:executing-plans. 用户已确认草案并要求先制定计划、继续实装；本轮由主 agent 连续实施，末尾一次只读审阅。

**Goal:** 在默认世界加入少量白细胞、通缉式免疫警觉度、贴身接触攻击及有限墙边增援。

**Architecture:** Rust 持有实体与行为状态，Rapier 持有身体并处理流体/接触，bridge 与 Godot 只增加显示数据。沿用现有共用细胞记录与模拟协调器；白细胞独立于红细胞远场数量池。

**Tech Stack:** 现有 Rust、Rapier 0.36.1、Godot/GDExtension；无新增依赖。

**Spec:** `docs/superpowers/specs/2026-10-30-white-cell-immunity-draft.md`，用户于本轮确认按墙边合法生成的游戏化方案实施。

## Global Constraints

- 留在当前分支，保留此前的生命/尸体代码；用户随后明确要求阶段性提交推送，不包含合并。
- 4 只初始白细胞，接触准备 0.3 秒、伤害 8、间隔 0.6 秒；警觉度 0～100，50 起局部识别通缉身份及呼叫资格。
- 呼叫持续感知 1 秒，每次最多 1 只，共享间隔 8 秒，白细胞身体总数上限 8；不新增队列或真正穿墙机制。
- 不增加浓度场、寻路、抓取、碎片、云实验、性能优化、物理子步或细胞节点数。
- 只补约 4～5 个覆盖关键行为的直接模拟测试；不复制 UI/bridge 测试、不建立通用 AI 测试框架。
- UI 验证只做加载、现有绘制检查和截图，不注入键鼠或执行自动交互。

## Review Focus

- 全局警觉不提供实时坐标；隔墙/出范围后只使用最后位置，低警觉无证据时中立。
- 玩家当步击杀白细胞后不再受其伤害，接触计时不按粒子重复，损毁只能重置恢复。
- 增援共享间隔、上限和管腔内合法位置有效；没有空位就跳过，不改物理墙和 LBM 边界。
- Local 模式仅汇总红细胞；白细胞身份、尸体、增援不污染红细胞数量统计。
- 不同体型继续进入现有流体路径；动态身体数变化需更新 Stokes 的已有标记历史存储。

### Task 1: Rust 行为与真实身体

**Files:** `cell.rs` 共用类型；新 `white_cell.rs` 局部行为/参数；新 `runtime/immunity.rs` 协调与生成；`runtime/gameplay.rs` 伤害衔接；`config.rs`、`runtime.rs`、`fluid/mod.rs`、`fluid/regions.rs`、`view.rs`、`lib.rs`。

**Interfaces:** 现有 MoveInput/tick/轮廓快照保留。增加 `WhiteCellOptions` 参数、类型及准备/呼叫显示状态、机器人生命与警觉度快照；流体仍接收软体句柄切片。Stokes 覆盖权重使用实际身体尺度；实体变化时清理已有力历史，不创建新的恢复流程。

- [x] 补少量真实模拟测试，复用现有直管/数量窗口夹具，覆盖有效伤害与警觉、感知/追逐、接触伤害及死亡、有限增援及数量隔离。
- [x] 实现巡游、局部调查/追逐、记忆、有限主动运动，以及共享警觉计分与安静回落。
- [x] 实现机器人生命、接触准备/冷却、损毁与重置，复用白细胞尸体逻辑。
- [x] 在合法贴壁位置初始化/增援；白细胞常驻现有物理世界，不进入红细胞数量池。
- [x] 保持 loop lab 与 LBM 专用测量场原种群；有固定种群契约的现有探针显式关闭白细胞，保留原有效断言。

Expected: 实际模拟可观测追逐、接触伤害、警觉变化和受约束的增援，旧探针与默认/feature 检查继续编译。

### Task 2: 表现、文档与集中验证

**Files:** `crates/godot_bridge/src/lib.rs`、`godot/world.gd`、`godot/world_hud.gd`、README、STATUS、本计划。

**Interfaces:** 消费 Task 1 的同序类型/行为元数据；Godot 只绘制白细胞配色、接触准备、呼叫和墙边入场提示，以及自机生命和警觉度。

- [x] 接入 HUD 与身体表现；保留 J 攻击和 R 重置操作。
- [x] 运行 `bash scripts/check.sh`、`cargo test --locked -p simulation --features lbm-gpu`、workspace all-targets feature check 及现有 OpenGL mesh 用例。
- [x] 查看默认世界截图；如有必要，仅用静态绘制夹具查看提示样式，不自动执行交互流程。
- [x] 一次独立只读审阅；只处理实际核心问题，按受影响层验证修正，不反复重启全量检查。
- [x] 更新操作说明、实际验证记录与人工体验限制。

Expected: 现有检查通过，普通场景能加载、截图可读；不声称完成手感或新战斗负载的实卡性能验收。

## 实施记录

计划已核对共享接口：Task 1 输出生物/战斗快照，Task 2 消费相同字段；数值集中在 Rust 参数中，表现层不决定伤害或生成。


## 实际验证与收尾

- `bash scripts/check.sh`：通过，包含 fmt、workspace check、现有模拟测试、扩展构建、Godot editor 导入以及 world/lab 短时加载；日志无脚本错误。
- `cargo test --locked -p simulation --features lbm-gpu`：通过；显式软件 GPU/硬件诊断仍保留 ignored，本轮没有运行。
- `cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`：通过，包含旧探针与示例。
- `DISPLAY=:2 LIBGL_ALWAYS_SOFTWARE=1 godot --path godot --audio-driver Dummy --script res://tests/world_cell_mesh_test.gd`：真实 OpenGL 上下文下通过。
- 默认世界第 120 步截图已查看：4 只白细胞、机器人生命与警觉 HUD 正常，控件无遮挡；未注入键鼠或执行自动 UI 交互。
- `git diff --check` 与一次独立只读审阅通过，未发现 Critical/Important 问题，未追加复审或测试矩阵。

本轮新增 5 个直接模拟测试，分别覆盖实际伤害与警觉/局部目击、隔墙后的最后位置记忆、贴身准备/中断/当步死亡及自机损毁、有限增援及红白数量隔离、停流下的真实主动移动。既有“空详细窗口”用例因默认世界现在有白细胞而失败，已在该固定零细胞夹具中显式关闭白细胞，保留原断言；固定种群探针同样显式关闭，不改变原测试预期。

实际接入采用 `WhiteCellOptions` 集中参数，`Cell.white` 只为白细胞携带行为状态，避免给红细胞分配空 AI 状态。身体成员改变时清空已有 Stokes 索引历史，现有标记收集同时匹配当前身体数量；没有增加身份映射框架或新的流体恢复流程。通用自定义 LBM 地图构造器不默认放置白细胞；普通世界入口启用，LBM 专用测量场保持原种群，重置保留场景选择。

人工手感、极端拥堵中的变形与顶墙、墙边入场效果及新战斗负载的实卡性能仍待后续体验，不自动安排云实验。共享间隔内呼叫进度不积累，拥堵/人数达到上限则消耗本次机会后跳过；这些是本版接受的简单规则。用户随后要求阶段性提交推送，生命/尸体与白细胞代码、文档一并归档至 `feat/godot-static-background`，不包含合并 `main`。
