# 四腔心脏几何预览实施计划

> For agentic workers: use superpowers:subagent-driven-development with user-selected sol medium implementers and sol high reviewers. Track steps with checkboxes.

**Goal:** 提供真实碰撞、可WASD穿行的四腔心脏几何预览。
**Architecture:** ChamberGeometry与已有管道共同提供权威SDF；复用分块碰撞、绘制和单Rapier推进。显式preview场景强制零流/零细胞，先验证空间，再另做流场及数量模型。
**Tech Stack:** 现有Rust/Rapier/Godot，不新增依赖。
**Spec:** [已确认设计](../specs/2026-10-31-heart-geometry-preview-design.md)；用户明确回复“可以”同意新增模型和本阶段范围，并已选定委派执行方式。

## Global Constraints

- 16单位块、5×5现场、zoom2、1280×800时26.7×16.7单位视野；不生成全图mesh。
- 每个心室主向可通行跨度≥80单位（5块、3个画面较长边），长轴目标约96单位，整体约160×192。
- 四腔闭合轮廓与管道并集；左右心组织隔开，窄口/流出道宽8～12，静态无阀门开合或单向机制。
- 只做零流/零细胞预览：pressure/pulse_hz/red count/white count固定0；WASD/暂停/单步/重置可用，HUD不得回写非零压差或显示伪造流向。
- 旧地图chambers为空，旧默认/Local/LBM路径保持；离线LBM已完成功能作为本分支基线，但本阶段不烘焙心腔。
- 现有GraphNetwork须合法连通并有外部闭环；零压图不作为心腔阻力验收。
- 不新增crate/依赖、库存系统、动态几何、缓存队列、编辑器、云/性能/64实时窗口实验。
- 用户测试约定优先于 blanket TDD；关键新行为少量真实测试，末尾集中检查。Godot只加载与截图，不自动键鼠/UI流程。
- 重要路线超出设计上报；不自动推送/合并。

## Review Focus

- 单改signed_distance而漏combined查询/bounds会导致墙与显示不符；Task1测试。
- 草图外观跨度不能代替玩家净空和可达跨度；Task2真实机器人回放。
- 组织中隔可能被连接管意外穿通；Task2局部心脏ROI连通/隔墙检查，不把外部正常循环当捷径。
- 调速会连带回写HUD压力；Task3桥接参数入口保护，Task2直接配置突变验证。
- reset要保留预览类型及零状态；Task2真实reset/暂停/单步测试，Task3启动状态。

## 文件及顺序

Task1→Task2→Task3，接口耦合顺序执行。先保存文档和计划，使用现有干净宿主工作树新分支feat/heart-geometry-preview；基于已完成的feat/offline-lbm-bake以包含最新基础设施，不动主目录main或旧分支提交。每任务本地提交，仅提交拥有的源文件和必要文档，SDD报告不force-add。

### Task 1: 独立心腔轮廓与统一几何

**Ownership:** map/chamber.rs(新)、map.rs、map/geometry.rs；所有既有MapDefinition字面量仅补chambers空集合。
**Interfaces:** map公开ChamberGeometry { name, boundary }及构造器/只读bounds（可存一次计算的派生bounds，不建空间索引）。signed_distance(&self,Vector)->f32支持简单凹多边形；MapDefinition.chambers: Vec<ChamberGeometry>。轮廓无重复闭合点也可正确闭合最后边。

- [x] 实现到边的最小距离与点内外符号，有限非退化轮廓的局部构造检查；不新增自交修复、洞/布尔编辑器。只读bounds是派生值，不另维护可编辑第二几何。
- [x] signed_distance与nearest_edge_and_signed_distance都合并心腔；nearest_edge仍保留既有图边归属。全图几何bounds包含心腔，按块采样与墙/地面生成复用现有实现。旧map构造和字面量chambers为空（含离线例子），旧数值不改。
- [x] 新增少量map真实行为测试：凹腔内部/凹口/边界符号、只在管道bounds之外的腔室仍有绘制/墙、combined查询与物理SDF一致、跨16块接缝不封口。复用现有map测试夹具，不复制SDF算法作期望。
- [x] 运行cargo test --locked -p simulation map::；feature编译仅必要一次，集中检查Task3。自审提交。

### Task 2: 四腔布局、零状态运行与真实穿行

**Ownership:** map/heart.rs(新)、map.rs、runtime/heart_preview.rs(新)、runtime.rs、lib.rs、examples/heart_preview_probe.rs(新)。
**Interfaces:** MapDefinition::heart_preview()->Self；Simulation::new_heart_preview(config:Config)->Self；is_heart_preview()->bool。一个私有preview标记保存模式，构造/step/reset统一固定零状态，原有实例默认false。不增加通用场景注册框架。

地图轮廓参考主目录.worktrees/heart-proportion-20261031/heart-proportion.html比例图；曲线控制点可局部调整以满足实际净空/跨度，不逐像素复制草图。新曲线采样用标准公式，旧bezier与小地图不顺带修复。MapDefinition.edges沿真实心腔/静态窄口与域外肺/体循环连接形成合法图，不能跨组织划捷径，内侧连接管尽量完整留在腔室内。

- [x] 构造4个具名腔室、左右房室窄口、同侧流出道与4个独立外部接口。左右之间有组织中隔；外部连线在心脏主体外闭合。心室实际可走主向跨度至少80，整体约160×192，目标长轴96。提供具名入口/深部/出口导航点供真实检查，landmarks用于认路，不把标记框作为墙。
- [x] new_heart_preview复用已有streamed构造和单Rapier，清零压力/脉动/细胞/白细胞；预览标记在step前和reset确保零状态，保留调速、暂停。默认工厂不受影响；preview类型在reset先于普通stream/policy分支处理。
- [x] 同一层少量真实测试：4腔内部与隔墙/连接口、心脏ROI的左右无捷径（排除外部正常回路）、配置压力/数量被改后仍零流零细胞、暂停/单步/reset保持预览地图。
- [x] 实现Rust输入回放probe：从正常初始位置以MoveInput按声明路径经过右房/右室深部/出口、肺外连接、左房/左室深部/出口。可用现有允许robot_speed=8作功能回放，不瞬移/穿墙；分段有界，记录实际位置、墙距离、两心室实际跨度、场景块数、零数量/流量。要求实际到达，不能只检查目标坐标/AABB；失败查根因。无需走完远处体循环长回路。
- [x] 运行聚焦heart相关CPU测试和probe；复用相同测试层，不新增大型验证框架。自审提交；完整默认/feature套件留Task3。

### Task 3: Godot显式入口与集中交付

**Ownership:** godot_bridge/src/lib.rs、godot/{world.gd,world_hud.gd}、必要docs/STATUS.md/计划和docs/research/2026-10-31-heart-geometry-preview/。
**Interfaces:** BloodLab::use_heart_preview()创建并设置stream_world=true；set_parameters中preview压力固定0。Godot --heart-preview隐式stream/zoom2，HUD.set_heart_preview()做模式标题与流体控件状态，不扩大snapshot协议。

- [x] 在_ready解析preview，在任何场景工厂之前拒绝与local-lbm/local-stokes/lbm-validation/profile/旧benchmark组合；stream-world/local-cells可视为冗余，最终preview配置优先。调用新factory，普通路径原样。预览HUD清楚显示模式、压力0且不可编辑，流向开关/F无效，拖曳控件可禁用；不存在profile下拉时不新增控件。保留speed/WASD/暂停/N/R/碰撞显示。
- [x] 参数回写在桥接和Rust标记处保证pressure0，reset后仍为preview。HUD调整遵循现有信号顺序，不只禁用鼠标却允许快捷键/参数信号恢复流向。非preview无差异。
- [x] 集中运行cargo fmt --all --check、cargo test --locked -p simulation、cargo test --locked -p simulation --features lbm-gpu、cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu及项目scripts/check.sh。新预览headless启动/不兼容参数的退出状态检查可用命令行；不做UI交互测试。
- [x] 真实渲染只截一张正常预览静态图，使用现有本机Xorg/GL；不得自动按键/点击/填写控件。必要时另从实际轮廓数据输出静态总览用于说明尺度，不把草图当游戏截图。若无可用图形环境如实记录未验收，不启动云实例。
- [x] 保存探针日志、命令结果和源码来源，更新STATUS和完成步骤。已知首次Godot导入134如再现，读日志并只重试失败导入/补跑未执行场景；不重跑整套Rust检查或添加引擎绕过。
- [x] 自审本地提交；
- [ ] 主agent派新sol high整分支审阅。本阶段完成只表示可探索几何预览，未接心腔流场/细胞，不自动推送/合并。

## 自检

新增几何/合并查询与bounds→Task1；尺度/拓扑/真实到达和零状态→Task2；实际入口、HUD回写和图像→Task3。已有流式地图与离线基础设施保持可编译；不在此轮实施正式心腔阻力/库存/烘焙。
