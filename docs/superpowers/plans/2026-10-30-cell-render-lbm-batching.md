# 细胞绘图与LBM合批 Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans in this session. 用户确认当前分支实施，随后明确要求阶段性提交推送；本次归档不启动云实例。

**Goal:** 复用细胞双层填色网格，按机械批合并LBM编码/提交，HUD仅10Hz。

**Architecture:** 沿用现有Godot表现层和Rust/wgpu模块，snapshot不变。每个可见细胞一个复用ArrayMesh，原抗锯齿轮廓仍由World绘制；LBM内部批次记录原有全部pass后一次submit。

**Tech Stack:** Godot 4.7.1 GL compatibility / GDScript、Rust、wgpu 27.0.1。

**Spec:** ../specs/2026-10-30-cell-render-lbm-batching-design.md

## Global Constraints

- 保留真实软体轮廓、当前视野剔除、颜色、0.48内层比例和原抗锯齿轮廓。
- 保留14格子步、两个机械批次和三次正常读回；不减少采样或改变均值控制。
- 无新依赖、桥接字段、默认地图/物理参数改变；保留已有研究记录。
- 按项目AGENTS集中运行相关检查；HUD不新增永久测试，不为低风险UI写测试框架。

## Review Focus

- 凹形及压扁轮廓：原生剖分一次，两层索引一致；不盲用固定三角扇。
- 细胞数量/节点数变化、离开视野、reset：不显示旧资源，按需复用或重建。
- Mesh顶点格式与动态索引更新：核对Godot实际API，避免按错误stride写缓冲。
- 非零且跨批变化的GPU参数、奇偶步population交替：与真实单步场等价。
- 物理暂停/单步控件立即更新，HUD数值100ms；headless仍推进模拟。

### Task 1: HUD降频

**Files:** godot/world_hud.gd

- [x] 增加100000usec墙钟间隔，仅门控统计/流量文本，控件状态在门控之前更新。
- [x] Godot cached import与180帧headless回放退出0；不新增永久HUD测试。

### Task 2: LBM合批

**Files:** crates/fluid-gpu/src/lbm/gpu.rs、crates/simulation/src/fluid/streaming_window.rs

**Interfaces:** LbmWindow::advance_batch(input: StepInput, count: usize) -> LbmResult<()>；advance委托count=1。StreamingWindow缓存/延迟刷新接口不变。

- [x] 扩展现有batched_advance_matches_individual_steps：非零跨批输入，4/3奇偶批次，完整宏观场比较，实际batch只新增一次submit。先观察旧实现失败。
- [x] 一次上传Params，一encoder按原次序记录全部collide/openings pass，正确翻转current，一次submit；StreamingWindow调用此路径。
- [x] 非法count/非有限输入在GPU写入前返回；复用现有相关用例。
- [x] 运行上述软件GPU用例及已有finish_frame/耦合用例；预期正常完整帧33dispatch、8submit、3readback。

### Task 3: Godot细胞网格

**Files:** 新增godot/world_cell_mesh.gd；修改godot/world.gd；Godot生成对应uid。

**Interfaces:** helper.update(cell: PackedVector2Array, scale: float, color: Color) -> bool，持有mesh与closed_contour；World持有按snapshot顺序的槽位，只更新可见细胞，draw_mesh后draw_polyline。helper仅处理真实填色几何，不持有模拟状态或身份。

- [x] 用临时Godot行为脚本验证凹形、两层几何、节点数变化、空/非法剖分不画旧网格；仅必要几何不变量保留为短测试。
- [x] 每帧原生triangulate一次；复用已分配顶点/颜色/索引，布局变化时重建surface，否则更新顶点及必要索引。
- [x] World先AABB剔除，按原顺序提交双层Mesh＋原轮廓，保持debug/机器人画法。无实际渲染时不创建Mesh。
- [x] 默认世界同帧截图并检查凹形/重叠，记录本地绘图CPU及draw calls；不把软件渲染数字当RTX验收。

### Task 4: 集中验证与交接

- [x] scripts/check.sh、LBM feature check及相关已有软件GPU用例，读取真实退出状态。
- [x] 一个fresh reviewer审阅本轮产品diff；不扩成无关理论风险修复。
- [x] 保存改动、实际检查命令/结果、性能证据及局限；更新STATUS；用户随后安排阶段性提交推送，纳入本次归档。
