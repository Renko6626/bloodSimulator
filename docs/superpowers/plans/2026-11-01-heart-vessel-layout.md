# 心脏分级血管布局实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development；沿用用户指定sol medium实施、sol high审阅。按复选框跟踪。

**Goal:** 将心脏预览的占位回路替换为真实可穿行的肺/体分级血管，并用生产图求解核对Q48参考流量。
**Architecture:** 沿用MapDefinition/VesselEdge与同一SDF、分块绘制/碰撞、GraphNetwork和heart-preview工厂；新增的私有辅助仅组织这张手工地图的重复分支，不建通用生成框架。两套1→4→16→64模块串联，保持零流/零细胞预览，参考流量仅在CPU检查中求解。
**Tech Stack:** 现有Rust/Rapier/Godot，无新依赖。
**Spec:** [已确认连接方案](../specs/2026-11-01-heart-vessel-layout-draft.md)。用户“开干”批准完整分级、Q48和候选端口为起点；无需重复确认相同路线。

## Global Constraints

- 两套循环串联，每套主干/支管/小管/微管数量1/4/16/64，直径24/12/6/3；回流镜像，微管每套64条，不把两套流量相加。
- H=1、参考循环Q48，目标单管流量48/12/3/.75，均速2/1/.5/.25；同组等效阻力对称，必须用真实GraphNetwork检查，禁止逐边覆盖速度或阻力作弊。
- 保留4腔轮廓、房室口及D9/10接口，左右组织隔墙、心室实际跨度≥80；四端口轴线x38/78/112/150、y=-32，心脏域[-32,224]²内只保留心脏及四条接口，不引入旧占位回路或外部血管。
- 主干连接y≤-64；肺模块向上，体模块向下，体循环外走廊x≤-64/x≥256、下部y≥272；实际范围随合法布局确定，不硬塞旧世界外框。尺寸均为world units。
- 固定16单位块、地图/实体5×5、实时LBM3×3、zoom2、单Rapier；只生成现场块，不全图mesh。
- 继续使用--heart-preview且零pressure/pulse/红白细胞；默认/Local/LBM路径不变。参考压差为图模型结果，不称真实心腔压力，不启用运行血流。
- 不新增依赖、公开地图/存储协议、通用生成器、管径模型、独立泵、库存、细胞填充、动态瓣膜、烘焙、性能或云实验；不得自行改已确认数量/管径/Q/端口来消除失败。
- 按用户测试约定，关键新增行为少量真实检查，末尾集中完整检查；不自动键鼠/UI流程；不自动push/merge。

## Review Focus

- 图连通但SDF管道相交会产生隐藏捷径：Task1核对外管非相邻段的净间隔及真实几何分离，不只看GraphNetwork。
- 同径支路长度不同会破坏均分：Task1用生产图解检查全部层级，而非理论表或只看总量。
- 旧心脏ROI flood和路线原来经过短肺连线：Task1保留隔墙/房室口测试并更新真实导航，不弱化断言；Task2实际穿行两套模块。
- 巨大世界不能提前生成全图碰撞：Task2完整回放现场块数与零状态，不用目标坐标/AABB冒充穿行。
- 后续烘焙域若含外围管道会多出开口：Task1核对心脏域内仅四条接口与四腔，并在报告中给来源总览。

---

### Task 1: 两套真实分级几何及图流量

**Ownership:** `crates/simulation/src/map/heart.rs`；可创建`map/heart/vascular.rs`私有心脏布局辅助并在heart.rs声明。必要的map.rs仅改本模块导出注释，不增加公开协议。既有heart map测试位于heart.rs，可拆本模块私有测试文件而不改全局测试架构。
**Interfaces:** 保持`MapDefinition::heart_preview()->Self`与`heart_preview_route()->Vec<(&'static str,[f32;2])>`签名。后者提供从正常spawn经右房/右室深部、肺模块一条完整微管支路、左房/左室深部、体模块一条完整微管支路再回右房的顺序中心线导航点。保留right_entry/right_deep/left_entry/left_deep名称供实际跨度测量；具名标记只选认路点，不给每个路线采样点画landmark。

- [x] 保留四腔曲线、隔墙、spawn及内部连接；替换旧肺短连线/外围矩形回路，延伸四接口。两套模块各106个逻辑定宽段，可加合法定宽连接/过渡段；4路逻辑分配可分散为多个几何分叉，不要求某种四岔形状。
- [x] 选择无交叉的私有布局，成组等长或等效阻力对称；微管并联通路必须几何独立，不重叠成一根管。全图可达，两套循环必须通过末梢后回流，外管不跨心脏或烘焙域造成捷径。考虑D24端帽范围，不只检查中心线。
- [x] 保持原房室口/隔墙测试；增加少量关键真实行为检查：全部等级数量与宽度、真实GraphNetwork单位压差归一化后各支路Q及守恒、外管非预期几何相交/分离、域内四接口。直接生产map/solver/SDF；对浮点求解使用合理小误差，不用松容差掩盖不等长。
- [x] 更新共享导航路线：转角前后中心点、点间距离使现有2400帧有界控制能到达；不得瞬移或降低机器人/墙半径。数值参考求解只用于检查，不给preview接背景或压力。
- [x] 运行聚焦`cargo test --locked -p simulation heart_preview --lib`及新增测试的匹配过滤；fmt/diff。完整套件留Task2。自审本地提交，仅提交拥有文件；报告实际边/节点/微管计数、bounds、参考压差和全部层级流量误差。

### Task 2: 穿行、总览与集中交付

**Ownership:** `crates/simulation/examples/heart_preview_probe.rs`、`docs/STATUS.md`、本计划及`docs/research/2026-11-01-heart-vessel-layout/`。只有发现地图路线具体缺陷并向主agent报告后才能请求Task1文件修正；不改Godot/HUD/其他工厂。
**Interfaces:** 复用Task1同名工厂与路线，zero Config和原有MoveInput；GraphNetwork公开new/solve及rates支持参考检查。可在现有诊断example加仅限诊断的SVG/CSV输出参数，不建生产导出API或通用工具。

- [x] 真实输入回放覆盖两心室、肺和体各一条完整微管路线并返回；沿用真实生产推进，不瞬移。实际跨度≥80，逐帧零数量/压差/流量，零细胞现场峰值25块，保守中心净空≥1.0（机器人radius.85不变）。失败找具体几何/转角原因；不为了通过删路线或松守卫。
- [x] 从实际map数据输出全图总览及心脏端口/一组微管细节（可SVG/PNG），清楚标注诊断图而非游戏截图，虚线不充当流场。保存实际参考图流量CSV、真实probe日志、源码来源。不要用理论flow-table.csv充当实际solver输出。
- [x] 集中运行`bash scripts/check.sh`（含fmt/default tests/build/普通world+lab load，不另重复默认套件）、`cargo test --locked -p simulation --features lbm-gpu`、`cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`，以及headless --heart-preview加载。CARGO_TARGET_DIR复用主目录target。冷Godot导入如134只补验失败/未执行场景，不重跑Rust套件或扩展引擎研究。
- [x] 无GDScript改动，无需重复UI截图/交互；只允许加载或静态截图，禁止自动按键点击。报告geometry+图参考流量验收，明确未接心脏流场/细胞/烘焙/实卡性能。
- [x] 更新STATUS和报告、计划步骤，完成自审本地提交；保持工作树，不push/merge。
- [x] 主agent派fresh sol high整分支审阅，Spec/Quality PASS，无发现；[最终审阅](../../research/2026-11-01-heart-vessel-layout/review.md)与[执行决定](../../research/2026-11-01-heart-vessel-layout/controller-notes.md)已归档。

执行结果：首次210点回放暴露最终右房返回路线遗漏(38,32)的真实折点，净空0.852失败。Task1在5ea9431补回并加入生产SDF段采样回归；修正后211点87771tick完整回放通过，最小实际净空1.273，峰值25，实际跨度右83.985/左89.943，逐帧零状态。默认47+5、feature60+5/10忽略及集中加载通过；路线修正后仅补受影响fmt/全目标编译/bridge构建/heart加载与完整回放。证据见[研究记录](../../research/2026-11-01-heart-vessel-layout/README.md)。逐任务、修正复审及最终独立整分支审阅均通过，无待修复发现。
