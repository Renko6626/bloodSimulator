# 心脏烘焙背景接入实施计划

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task. 用户已于2026-11-03确认设计并要求“开干”，直接执行，不重复请求流程确认。

**Goal:** 在新器官地图中生成并正常导入心脏附近的真实LBM背景，区域之外使用二维解析流。

**Architecture:** 复用固定正方形离线LBM、单份时间平均资产、现有背景策略与端口校验。先通过实际资产四口校验，再接CPU运行入口；本阶段无细胞、无实时LBM。

**Tech Stack:** 既有Rust、wgpu/WGSL、Godot；无新依赖。

**Spec:** `docs/superpowers/specs/2026-11-03-organ-traffic-design.md`

## Global Constraints

- 地图尺度/路网/四腔不变；16单位块、5×5现场、单Rapier世界保持。
- 参考串联Q48，黏度0.05；新策略地图使用H=1。
- 所有预计算块共用FieldId；其他块解析；所有块禁止实时LBM。
- 不放宽端口容差、不替换求解速度、不启动云或性能实验。
- 作业有明确模拟预算和墙钟上限；失败留证据，不自动延长预算。
- 遵守AGENTS.md的测试范围；不为低风险静态配置添加测试，不自动交互Godot。

## Review Focus

- 斜管资产必须真正通过原逐点剖面/积分通量校验，不能只看总流量。
- 求解域可能含外围管段：逐分量检查开口收支，不能把额外分量误称心腔。
- 标签边界须只有四个合法流体接缝，完整覆盖腔体与插值支持。
- 运行不编译GPU也能读取资产；reset保留场景/压力/暂停状态。
- 新入口无红白细胞，调压不越过已声明范围；旧预览语义保持。

### Task 1: CPU预检与有界烘焙例子

**Files:** Create `crates/simulation/examples/organ_heart_bake.rs`；报告保存到`docs/research/2026-11-03-organ-heart-flow/`。

**Interfaces:** 消费`MapDefinition::organ_world`、`GraphNetwork::solve`、`rasterize_domain`、`bake_mean`、`to_precomputed_field`、`resample_precomputed_field`与原资产读写/策略世界。输出实际资产及机器可读来源与文本诊断。

- [ ] 增加显式`--preflight`与`--bake`，必须给输出目录；软件作业显式`--allow-software`。
- [ ] CPU预检候选标签、端口、方域、所有开口/连通分量、最大初场速度；记录生产参考压差。
- [ ] 预检后固定dt、逻辑步、预热/采样/stride及墙钟上限，写入作业说明再执行。
- [ ] 一次有界作业导出原场和重采样场，实际读回并走原策略世界校验；失败退出非零，保留完整报告，不开展Task 2。

验证：`cargo run --locked -p simulation --features lbm-gpu --example organ_heart_bake -- --preflight --output <DIR>`；GPU作业外层使用`timeout`明确最长用时。预期预检退出0；资产作业只有正常生产导入成功才退出0。

### Task 2: CPU背景运行入口（依赖Task 1成功）

**Files:** Create `assets/flow/organ-heart.bfld`及小型`crates/simulation/src/map/organ_flow.rs`；Modify runtime、lib导出（如需要）、godot_bridge、world.gd、world_hud.gd。

**Interfaces:** `Simulation::try_new_organ_world(Config) -> Result<Self,String>`；bridge `use_organ_world() -> bool`；Godot `--organ-world`。

- [ ] 将验收资产嵌入Rust，在现有MapFluidDefinition装配标签/参考驱动，走`try_new_policy_world`。
- [ ] 本阶段强制红白细胞为0；reset保留该场景、用户压力和暂停状态。
- [ ] 以一个真实生产构造/reset行为测试覆盖新入口、零细胞和资产加载；不重复所有原端口坏输入用例。
- [ ] 接入flag冲突、实际压力范围、背景阶段文案及正常流向显示，不改旧预览。

验证：新增行为测试、默认构建，Godot headless加载/静态截图；代表性有限跨来源边界检查。不使用键鼠自动交互。

### Task 3: 集中验证、证据和整分支审阅

**Files:** 阶段README、controller-notes、source-manifest，更新STATUS；必要的已有脚本不扩大CI。

- [ ] 资产静态速度/流线图与四口内外采样，报告近壁法向速度与心腔意外零场。
- [ ] 集中运行`cargo fmt --all --check`、默认simulation测试、feature测试及`cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`；Godot默认/新入口/预览短加载。
- [ ] 保存真实命令、结果、来源和未验收项；最终独立只读代码审阅；只修实际重要问题。
- [ ] 保留功能分支和工作树；未经要求不合并/推送，不声称交通/物理精度/FPS已验收。

## 执行记录

计划按原生执行方式实施，由主agent完成；末尾一次独立审阅。主工作树仅有设计草案，工作位置为宿主管理`7c02`独立工作树、`feat/organ-heart-flow`。
