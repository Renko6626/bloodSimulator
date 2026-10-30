# 离线LBM基础设施实施计划

> For agentic workers: use superpowers:subagent-driven-development, user-selected sol medium implementers and sol high reviewers; track steps with checkboxes.

**Goal:** 共用现有LBM生产可读回的时间平均PrecomputedField。
**Architecture:** 固定正方形域核心复用现有shader；移动窗口保留兼容包装；GPU层计算平均值，simulation负责参考条件与资产读写。
**Tech Stack:** 现有Rust/wgpu/WGSL及std::io，无新依赖。
**Spec:** [设计记录](../specs/2026-10-31-offline-lbm-bake-design.md)；用户已明确选择路线并要求开始写基础设施。

## Global Constraints

- 同一D2Q9 MRT/Guo/LSQ-FD/均值控制，不复制shader或另起Stokes实现。
- 实时16单位块、48×48窗口、5×5现场及默认入口不变；首版离线固定正方形域。
- 仅单份时间平均背景，驱动缩放是玩法近似；不得声称心腔物理精度或统计收敛。
- 不新建crate/依赖/后台任务/恢复队列，不改变PrecomputedField既有语义。
- 不启动云实例、硬件/性能研究、64×64实时移窗实验、心腔几何或UI交互。
- 用户测试约定优先于一律TDD/每任务全套检查；针对关键行为，末尾集中验证。
- 重要新架构决定上报，局部细节可明确记录；不自动推送或合并。

## Review Focus

- 旧窗口公开接口和重叠迁移必须仍工作；Task 1保留已有测试。
- 格数/格距/字节数与设备限制不匹配应报错；Task 1检查新输入。
- cell-center原点、掩码和速度顺序不能错位；Task 2/3真实读回与背景检查。
- 非有限求解或读回不能产出“成功资产”；Task 2错误传播。
- 有界采样预算不等于收敛验收；Task 3报告实际时长/样本/变化，不用模拟帧数冒充心腔验收。

### Task 1: 共用固定域核心，保留移动窗口

**Ownership:** fluid-gpu/src/lbm/{mod.rs,gpu.rs,model.rs,domain.rs(新),window.rs(新),tests.rs}；仅必要局部触及stencil.rs，不能复制它。
**Interfaces:** DomainSpec { origin:[f32;2], cells_per_side:u32, spacing:f32 }，validate/world_bounds；WindowSpec::domain()->DomainSpec。LbmDomain::new(DomainSpec,WindowConfig,InitialField,WindowGeometry)->LbmResult<Self>，复用现有推进/采样/均值控制API；LbmWindow保留原WindowSpec公开接口及shift_to。

- [ ] 把现有gpu实现改为接受DomainSpec的单份LbmDomain，保留全部原碰撞/边界代码；LbmWindow以兼容包装映射构造、spec及迁移。不要用可改变域原点的DerefMut暴露核心而让窗口spec失配；普通委派即可。
- [ ] 模型初始化/参数/开口构造改用域的格距和格数，正方形LSQ索引保持；原点迁移要求同格数/格距，不能静默重缩放。
- [ ] 校验新尺寸/有限范围/分配与设备限制，失败明确返回；不增加自动降分辨率。
- [ ] 小型CPU契约测试覆盖WindowSpec映射和非法固定域；运行现有fluid-gpu LBM CPU测试及simulation feature编译。报告哪些GPU测试默认ignored，不机械重跑全部软件测试。提交并自审。

### Task 2: 有界烘焙、转换和版本化读写

**Ownership:** fluid-gpu/src/lbm/bake.rs(新)、mod.rs；simulation/src/{offline_lbm.rs(新),lib.rs,map.rs,map/fluid_asset.rs(新)}。
**Interfaces:** BakeSchedule { steps_per_frame:usize,warmup_frames:usize,sample_frames:usize,sample_stride:usize }；bake_mean(domain:DomainSpec,config:WindowConfig,initial:InitialField,geometry:WindowGeometry,schedule:BakeSchedule)->LbmResult<BakedFlow>。BakedFlow含domain、velocity、fluid、BakeReport；报告样本数、时间、密度/通量及前后半均值差。提供CPU可测平均累加逻辑，不搭通用工作流框架。

simulation::offline_lbm提供把BakedFlow及geometry_revision/reference_pressure/max_scale/FieldPort转换为PrecomputedField的函数，不向GPU核心引入地图依赖。map::fluid_asset公开read_field(reader:impl Read)->Result<PrecomputedField,String>与write_field(writer:impl Write,field:&PrecomputedField)->Result<(),String>。

- [ ] 显式循环预热帧、采样帧，每帧推进后执行均值控制；间隔采样f64累加并导出平均值。预算及样本数检查；错误直接传播。变化量报告不用于自动声称收敛，不建立自适应重试状态。
- [ ] 转换时origin=domain.origin+spacing/2；fluid=!solid，现有参考条件由simulation调用者提供，保留原字段语义。
- [ ] 单文件格式：magic八字节BFLOW001；按顺序小端u64 geometry_revision、f32 viscosity/reference_pressure/max_scale、f32 origin[2]/spacing、u32 size[2]/port_count；每port为u32 edge、f32 along/outward_sign/reference_flux；随后行优先f32速度对和0/1掩码。检查整数转换/维度/精确负载和结构值，不替代原背景语义验证，不引入新依赖。
- [ ] 新增真实读写往返/截断或错误版本拒绝测试，独立平均表/样本计数测试；验证两种feature均编译。局部测试后提交，完整模拟检查留Task 3。

### Task 3: 直管闭环及集中验证

**Ownership:** simulation/examples/offline_lbm_bake.rs(新)、fluid/lbm_geometry.rs局部域适配、lib.rs必要导出；docs/STATUS.md、计划及docs/research/2026-10-31-offline-lbm-bake/。
**Interfaces:** 保留rasterize(map,flow,WindowSpec)并复用到rasterize_domain(map,flow,DomainSpec)；必要时opening_profiles同样增加固定域入口，旧路径包装保持。CLI仅显式例子：--output DIR --allow-software --functional，拒绝未成对的软件许可。

- [ ] 建立一个远处闭合回路、烘焙域内只有长直管的固定夹具；域80×80、h=.5即160²格，下界[-16,-16]，参考管中心y=24、半宽3，节点/回程在域外；Precomputed标记x=0/1,y=1两块，端口x=0和32留直管缓冲。只是支持数据管线的小例子，不改默认地图。
- [ ] 用GraphNetwork参考流量提供原开口；选择符合现有格子速度预算的dt及有限采样预算，记录具体值。离线80域与实时48窗口不同，不实施64实时窗口实验。
- [ ] 短程软件GPU作业写入bfld文件和明文报告，再读回，在相同地图安装到MapFluidDefinition，调用try_new_policy_world及背景采样。正常配置原校验必须通过，不能放宽端口/掩码/质量检查；若失败先诊断，不删除断言或伪造速度。记录实际结果和限制。
- [ ] 集中执行cargo fmt --all --check、cargo test --locked -p simulation、cargo test --locked -p simulation --features lbm-gpu、cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu及受影响fluid-gpu测试；仅一次本地软件功能作业，不启动云或完整UI场景。
- [ ] 保存必要输出、源码版本/哈希和命令结果；不把未收敛的短作业称为心腔验收。更新STATUS、完成步骤、自审提交，主agent随后做最终整体审阅。

执行方式沿用用户已选subagent-driven：每任务sol medium实现、sol high审阅，最后新sol high整分支检查。默认游戏、器官几何、性能研究及发布流程不在交付范围。
