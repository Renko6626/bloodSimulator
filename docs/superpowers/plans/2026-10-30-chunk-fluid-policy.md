# 区块流体策略实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, or superpowers:subagent-driven-development only if the user selects delegated execution. Read the spec and this plan together. Steps use checkbox syntax for tracking.

**Goal:** 地图按16单位区块声明背景流场和LBM覆盖许可，提前检查完整48×48窗口，并让角色在许可区与禁区间使用一致的基础流动。

**Architecture:** MapDefinition保存可选的源策略；simulation内背景模块负责二维解析/静态场采样，feature内模块生成有效窗口中心。runtime每固定帧选择唯一窗口，并统一背景拖曳与有限阻力耦合权重；不新增求解器或Godot侧策略。

**Tech Stack:** 现有Rust、Rapier、blood-fluid与可选blood-fluid-gpu；不新增依赖。

**Spec:** [已确认设计](../specs/2026-10-30-chunk-fluid-policy-design.md)。用户2026-10-30确认，包括二维流量口径、地图驱动上限、预计算限制和单窗口交接。

## Global Constraints

- 基础块16×16；一个LBM窗口覆盖3×3块即48×48，Fine为192²格；现场仍5×5块。负坐标向下取整，坐标使用已有ChunkId。
- 仍只有一个局部LBM窗口和一个Rapier世界。区块不各自拥有求解器，也不独立推进身体。
- 默认策略为解析背景、禁止LBM；源标签常驻，派生有效中心不能再手工编辑。
- 新策略地图采用深度D=1的二维流量；旧地图没有策略定义时保持原行为。
- 地图声明最大基础压差，准备取1.15倍覆盖脉动；越界在任何环境、实体、GPU推进前报错。
- 首版预计算仅支持固定黏度和端口流量比例下非负共同缩放，预计算区禁止LBM。
- LBM固定帧选择窗口，退出释放、返回重建；空间交接复用outer_band默认4单位，不保留离场涡旋。
- Aggregate/Hibernate、现有子步、MRT/LSQ-FD/均值控制不改；旧固定负载和实验默认不变。
- 重要边界变化须回报用户；心腔几何及场生成算法不是本计划任务。不新增后台队列、恢复机制、自动重算、云实验或profiling。
- 只为关键新行为新增少量真实逻辑测试；集中执行相关已有检查。UI最多截图，不自动键鼠。

## Review Focus

- 压差越界或黏度/步长变化：在模拟时间、身体、流体推进前返回错误；Task 2覆盖。
- 禁区出生或在禁区reset：保留LBM请求，稍后进入许可区能真正创建窗口；Task 5覆盖。
- 负坐标、有效区外沿和内部块边：只在有效区域外沿降权，整帧不转入未经检查窗口；Task 3/5覆盖。
- 同一静态场跨块及端口比例失配：采样连续，失配在准备时明确拒绝；Task 2覆盖。
- 预先拒绝候选与GPU求解错误：前者正常使用背景，后者继续明确报错，不能悄悄变成无LBM；Task 4/5核对现有错误传播及真实回放。

---

## 文件及依赖安排

| 文件 | 职责 |
| --- | --- |
| `crates/simulation/src/map/fluid_policy.rs`（新） | 源策略及静态资产纯数据 |
| `crates/simulation/src/background.rs`（新） | 准备/驱动检查、资产验证、统一背景采样 |
| `crates/simulation/src/fluid/lbm_policy.rs`（新，feature内） | 候选检查、拒绝原因、有效区外沿权重 |
| `crates/simulation/src/runtime/policy.rs`（新） | 新策略入口、帧边界窗口选择、reset辅助 |
| `map.rs`、`lib.rs`、`circulation/graph.rs` | 声明字段/模块和二维管流采样，保持旧接口 |
| `fluid/lbm_geometry.rs`、`lbm_coupling.rs`、`streaming_window.rs` | 共享二维深度、全局交接系数、显式目标原点 |
| `runtime.rs`、`runtime/immunity.rs` | 统一所有速度/力路径，保留一个物理推进点 |
| `lbm_validation.rs`、`examples/lbm_gameplay_probe.rs`及其同名子目录`policy.rs`（新） | 显式策略回放；旧探针默认路径保留 |

Task 1→2→3→4→5顺序执行，共用接口较多，不建议多个实现者同时修改runtime。测试放所属模块现有`#[cfg(test)]`中，不另建测试工程。实施前先把相关准备文档记录为独立本地文档提交，再由该提交创建隔离工作树，不纳入无关工作区改动。实现按任务形成可审阅本地提交；推送/合并由用户后续安排。本轮先写计划，不在主工作区开始产品改动。

### Task 1：源标签与地图兼容

**Files:** 创建`map/fluid_policy.rs`；修改`map.rs`、已有构造MapDefinition的`lbm_validation.rs`、示例和测试字面量。

**Interfaces:** `MapDefinition.fluid: Option<MapFluidDefinition>`；`map`公开`fluid_policy`子模块。定义`FieldId(usize)`、`BackgroundSource::{Analytic, Precomputed(FieldId)}`、`ChunkFluidPolicy { background, allow_lbm }`。`MapFluidDefinition { geometry_revision: u64, max_base_pressure: f32, chunks: BTreeMap<ChunkId, ChunkFluidPolicy>, fields: Vec<PrecomputedField> }`；`policy_at(ChunkId) -> ChunkFluidPolicy`缺省返回解析/禁止，无额外可修改的默认许可。

`PrecomputedField`包含`geometry_revision: u64, viscosity: f32, reference_pressure: f32, max_scale: f32, origin: [f32;2], spacing: f32, size: [usize;2], velocity: Vec<[f32;2]>, fluid: Vec<bool>, ports: Vec<FieldPort>`。origin为首个采样点的世界坐标，数据行优先；资产边缘需覆盖插值支持。`FieldPort { edge: usize, along: f32, outward_sign: f32, reference_flux: f32 }`的截面由血管在along处的位置/切线/半宽决定，outward_sign仅±1，reference_flux为有符号二维外向通量。

- [ ] 新增源数据及`policy_at`；旧地图及旧字面量显式`fluid: None`，不改变几何或流量参数。源策略不依赖GPU或Godot。
- [ ] 在`map/fluid_policy.rs`增加一个查表行为测试：世界点`[-0.1,16]`落在`ChunkId{-1,1}`；覆盖此块为预计算/禁止，未配置邻块仍解析/禁止。测试调用生产ChunkId与policy_at，不只断言常量。
- [ ] 运行`cargo test --locked -p simulation map::`；预期旧几何与新查表通过。用`rg -n 'MapDefinition \{' crates`核对所有字面量已补字段，不能把测试字面量改成新策略而变更旧负载。

### Task 2：背景采样、资产契约及驱动检查

**Files:** 新建`background.rs`；修改`lib.rs`、`circulation/graph.rs`、`fluid/lbm_geometry.rs`。

**Interfaces:** 内部`PreparedBackground::new(map: &MapDefinition, config: &Config, unit_flow: &GraphFlow) -> Result<Self,String>`，`validate_step(&self, config: &Config) -> Result<(),String>`，`sample(&self, map: &MapDefinition, flow: &GraphFlow, instantaneous_pressure: f32, p: Vector) -> Vector`；unit_flow来自同黏度`GraphNetwork::solve(1.)`。公开管流函数`graph::sample_planar_velocity(position: Vector, map: &MapDefinition, flow: &GraphFlow, depth: f32) -> Vector`，原`sample_velocity`行为保留。

- [ ] 实现二维抛物线采样，复用现有投影与节点混合逻辑。直管不再叠加会改变截面通量的额外衰减；旧圆管采样保留原衰减。把`EQUIVALENT_DEPTH=1`移至无GPU依赖的background模块，lbm_geometry继续重导出原路径，避免默认feature引用可选crate。
- [ ] 实现静态场双线性采样与固体零速度；同一资产用世界坐标采样，与显示块加载无关。采样选择源标签后调用对应背景；使用当前瞬时压差/reference_pressure缩放。
- [ ] 在new中检查驱动上限、有限网格/数据尺寸、掩码和权威地图几何一致、geometry_revision/黏度/参考驱动/α范围、资产覆盖及插值支持。Precomputed块显式allow_lbm=true或未知FieldId报错。用unit_flow核对各端口reference_flux及方向，检查1.15×最大基础压差可共同缩放；预计算与解析只在声明直管端口连接，核对端口速度剖面与二维通量，其余外围为固体。数值容差依据采样分辨率选取并写在校验函数旁，不通过放宽容差掩盖明显错配。
- [ ] 实现validate_step：基础压差有限且0≤pressure≤上限；黏度、dt、substeps与准备值相同。不在此层推进环境或求解，调用方必须在任何推进前执行。
- [ ] 新增少量背景测试：R=2、Q=8、D=1直管中心3、偏移1处2.25、壁面0，数值积分通量约8；独立2×2向量表中点插值为[1,2]并按α缩放；共享资产跨x=16采样趋向同一值；坏端口参考通量与越界驱动拒绝。夹具调用真实生产采样/校验，不用源码匹配或mock。运行`cargo test --locked -p simulation background`及旧`circulation::graph::tests`，预期新契约通过、旧解析行为保持。

### Task 3：完整窗口检查及空间权重

**Files:** 新建`fluid/lbm_policy.rs`，在`fluid/mod.rs`按feature声明；仅在必要时给`lbm_geometry.rs`增加可复用的输入速度检查接口。

**Interfaces:** `PreparedLbmPolicy::new(map: &MapDefinition, peak_flow: &GraphFlow, options: &LbmOptions) -> Result<Self,String>`；`contains(&self, center: ChunkId) -> bool`；`activation(&self, player: Vector, band: f32) -> f32`；诊断`rejections(&self) -> &[LbmPolicyRejection]`，记录中心和原因。peak_flow来自1.15×地图最大基础压差。内部有效中心是BTreeSet，不缓存每个候选完整栅格。

- [ ] 只从显式许可块枚举中心，要求整个3×3均为解析且许可；缺少许可不进行栅格化。复用真实profile的rasterize与opening_profiles，保留所有已有拒绝理由，逐个检查后释放临时数据。几何拒绝记录原因并排除中心；背景配置错误由Task 2提前报错。
- [ ] 排除窗口闭合边界与等效泵段流体几何相交的候选，使用管段加半宽对AABB的保守相交判断，不能只看泵段端点。检查初场/开口峰值不超过velocity_bound；不据此宣称耦合运行稳定。
- [ ] 从有效中心块并集构造外边界线段，消去两个有效块之间的共享边；activation使用到这些外边界（含孔洞）的内侧距离，按band归一化smoothstep。集合外返回0，内部块边不降权；不新增距离场或时间状态。
- [ ] CPU侧feature测试复用现有LBM几何夹具：9块许可齐全才有有效中心；坏角点/泵段交叉保守拒绝；跨相邻有效块内部边界权重不降，外边界及负坐标位置正确。这些测试不创建GPU。运行`cargo test --locked -p simulation --features lbm-gpu lbm_policy`，预期通过。

### Task 4：单窗口生命周期与统一身体作用

**Files:** 新建`runtime/policy.rs`；修改`runtime.rs`、`runtime/immunity.rs`、`lib.rs`、`fluid/lbm_coupling.rs`、`fluid/streaming_window.rs`。

**Interfaces:** `Simulation::try_new_policy_world(config: Config, map: MapDefinition) -> Result<Self,String>`；feature内`try_new_policy_lbm_world(config: Config, options: LbmOptions, map: MapDefinition) -> Result<Self,String>`。两者均启用现有streaming现场并要求map.fluid为Some。Environment::World保存`background: Option<PreparedBackground>`；Simulation另存可选`PolicyRuntime`，其feature字段保存LBM请求配置与PreparedLbmPolicy，实际窗口仍使用现有`Simulation.lbm: Option<LbmCoupling>`。

`PolicyRuntime::select_frame(&self, position: Vector) -> Option<ChunkId>`只查已批准中心。新增`StreamingWindow::new_at(map: &MapDefinition, flow: &GraphFlow, origin: WindowOrigin, profile: GridProfile, config: WindowConfig) -> Result<Self,String>`、`maybe_shift_to(&mut self, map: &MapDefinition, flow: &GraphFlow, origin: WindowOrigin) -> Result<bool,String>`，以及`LbmCoupling::new_at(map: &MapDefinition, flow: &GraphFlow, config: &Config, options: LbmOptions, origin: WindowOrigin) -> Result<Self,String>`、`prepare_at(&mut self, map: &MapDefinition, flow: &GraphFlow, origin: WindowOrigin) -> Result<(),String>`。原new/prepare接口保留为旧调用的包装。窗口原点从中心减一块推导，绝不手填48或16。

诊断接口`Simulation::policy_fluid_status() -> Option<PolicyFluidStatus>`，类型由lib重导出，字段为`center: ChunkId, background: BackgroundSource, window_active: bool, activation: f32, rejection: Option<String>`。无策略地图返回None；策略地图没有活动窗口也必须返回Some诊断。

- [ ] 增加新构造入口，按Task 2/3先准备再构造身体，不因出生点无效创建窗口；用户请求仍存PolicyRuntime。复用已有build_seeded/streaming::local_config和墙体/种群初始化，让enable_streaming辅助函数可供无GPU入口使用，不重构全部工厂。旧接受自定义map的入口遇到Some(fluid)时明确指向新fallible入口，不能静默忽略策略；无需给Godot新增入口或旗标。
- [ ] Environment::velocity_at对新地图调用PreparedBackground，旧地图仍调用原graph采样。将固定步开始的驱动检查放在environment.update、寿命/数量、AI及GPU之前；瞬时驱动使用已有flow.summary.pressure_drop供统一采样，不另存一份权威压差。既有旧入口不额外受新上限约束。
- [ ] 帧开始选择中心，必要时创建/迁移/释放窗口；子步用prepare_at更新已锁定原点的开口驱动。上一帧finish完成后才允许更换窗口；原子步内按角色位置maybe_shift仅保留在旧入口。实际创建/推进错误直接传播，既有错误字符串可加上下文，不增加catch后回退。
- [ ] 给LbmCoupling增加默认1的frame_activation；策略帧设置为Task 3计算的a，local返回a×原空间权重。exchange反向冲量、细胞背景拖曳1-w、机器人/免疫flow_velocity_at、snapshot箭头及sample_world统一使用该local结果；新增actor不重设权重，不重复施力。无窗口时sample_world对策略入口仍返回有效背景及零局部权重，旧入口无窗口时的None语义保留。
- [ ] try_reset先按PolicyRuntime保存的请求重建，再处理现有white/paused状态；try_reset_lbm_profile也先识别策略请求，即使此刻没有窗口仍可更换profile并重建候选集合。不能凭当前lbm.is_some判断是否请求过LBM。保留原种群/地图/卸载策略。lbm_stats/work_stats无窗口时仍None；policy_fluid_status供探针读取，不扩大Godot snapshot协议。
- [ ] 在runtime/policy.rs加真实CPU模拟测试：越界调压try_tick返回错误且机器人、模拟时间、细胞寿命不变；有效范围能推进且reset仍保留策略；跨帧选择只查询已批准中心。复用现有模拟断言，不新建mock运行框架。运行`cargo test --locked -p simulation`及`cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`，预期默认和feature均编译/通过。

### Task 5：实际启停回放与交付记录

**Files:** 修改`lbm_validation.rs`及`examples/lbm_gameplay_probe.rs`；创建`examples/lbm_gameplay_probe/policy.rs`，更新`docs/STATUS.md`和本计划，结果存`docs/research/2026-10-30-chunk-fluid-policy/`。

**Interfaces:** `lbm_validation::policy_map() -> MapDefinition`从原验证场复制后加标签：仅x=0..2、y=0..2这9块许可，唯一有效中心(1,1)，出生点改为[14,24]（禁区中心）。max_base_pressure取该验证场原config().pressure；旧map/config/centers和旧探针默认不改。新`--fluid-policy`旗标调用独立`policy::run(args: &[String]) -> Result<(),String>`分支，使用Task 4新构造入口与status，不要求每帧都有LBM snapshot。

- [ ] 新回放复用现有quality/guard和MoveInput位置反馈：60帧禁区保持，随后每180帧依次向[24,24]、[14,24]、[24,24]移动，合计600帧。使用正常Rust模拟输入，无瞬移、无UI操作；记录实际中心/窗口创建消失、activation、marker力、速度差和数量账。若接触导致未完成实际进入/退出，明确失败，不能仅根据目标位置宣称交接完成。
- [ ] 软件GPU功能回放采用现有72红细胞及流式Aggregate，正常白细胞开关与新普通地图入口一致；检查真实创建→释放→再创建、禁区无窗口/零marker作用、机械守卫和数量账。另用已有ignored功能测试的方式覆盖禁区reset仍保留请求并能进入；不把软件功能结果称为实卡性能或物理精度验收。不新增自动UI交互。
- [ ] 执行一次集中验证：`bash scripts/check.sh`；`cargo test --locked -p simulation --features lbm-gpu`；`cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`。只在新改动或失败时重复受影响检查，不反复全量运行。
- [ ] 执行`cargo run --locked -p simulation --features lbm-gpu --example lbm_gameplay_probe -- --fluid-policy --allow-software --functional --frames 600 --output /tmp/blood-chunk-fluid-policy`，预期退出0且日志含实际启停观察。若本机无软件适配器，报告这项未验证，不启动云实例。执行前确保实现的新分支遵守frames=600；不沿用旧720帧每腿逻辑。
- [ ] 进行一次完整改动只读审阅，确认无重复推进、无隐藏回退、旧负载入口不变；按用户约定只修核心问题。保存必要日志、来源版本和限制，更新STATUS；列出实际命令和结果。心腔生成/大地图内容未实施，未测实卡FPS及长程混合稳定性。按任务本地提交；不自动推送、合并或改动部署。

## 计划自检及交付边界

源数据/默认许可→Task 1；二维换算/资产/驱动范围→Task 2；9块检查/泵段/外沿→Task 3；完整生命周期/采样和受力/reset→Task 4；实际启停与归档→Task 5。接口名在相邻任务保持一致，新增测试均对应设计中的关键行为。

本计划交付可使用的区块流体基础设施与显式Rust功能回放，不改变默认游戏地图，不完成心腔几何或离线场生成。后续器官地图待这些内容有独立设计后再接入，避免把接口实现扩展为另一轮流体研究。

状态：计划已编写，尚未开始代码实施；等待用户审阅及选择执行方式。建议本会话由主agent顺序实施，末尾一次独立审阅，减少强关联接口的交接成本。
