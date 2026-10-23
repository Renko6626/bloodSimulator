# 世界小地图实施计划

> 按 superpowers:subagent-driven-development 分工实施，用户指定子 agent 为 gpt-6-sol medium。用户已确认设计并要求开始，沿用该授权连续执行。进展在本文记录。

**目标：** 连续分岔闭环、两个器官样板、真实软体交通及跟随镜头。

**架构：** Rust 内置地图生成共享几何；纯阻力图输出流量，独立采样函数输出局部速度；同一个 runtime 推进原实验场或世界场景。Godot 分开世界、镜头与 HUD。

**技术栈：** 保持 Rust、Rapier 0.36.1、godot-rust 0.5.5、Godot 4.7，无新增依赖。

**设计依据：** [已确认设计](../specs/2026-10-22-world-small-map-design.md)。

## 约束与分工

- 原地保留既有未提交工程，不提交或覆盖无关成果。每个 worker 只能修改自己负责的文件。
- 先完成本批；战斗、区域装卸、真实流体、编辑器和磁盘格式后续单独推进。
- 使用已有检查并补关键行为缺口；不增加交互自动化或大规模验证。
- 图形最多截图，不能自动操作移动、按钮或控件。
- shared 类型变更先交主 agent 协调；新增重要路线决策先回报，不自行扩大范围。

## 固定接口

`simulation::map` 使用以下公共数据（坐标均为模拟单位）：

```rust
pub struct VesselEdge {
    pub from: usize, pub to: usize,
    pub centerline: Vec<[f32; 2]>, // 含首尾；构造时取 nodes 中端点坐标
    pub half_width: f32,
}
pub struct Landmark { pub name: String, pub position: [f32; 2], pub size: [f32; 2] }
pub struct MapDefinition {
    pub nodes: Vec<[f32; 2]>, pub edges: Vec<VesselEdge>,
    pub pump_edge: usize, pub spawn: [f32; 2], pub landmarks: Vec<Landmark>,
}
pub struct EdgeProjection {
    pub point: Vector, pub tangent: Vector, pub distance: f32, pub along: f32,
}
pub struct MapGeometry {
    pub walls: Vec<Vec<[f32; 2]>>, // 连续折线或双端点线段，均为实际墙中心线
    pub floor_vertices: Vec<[f32; 2]>, // 每三个点一组三角形，与边界共用交点
    pub bounds: [[f32; 2]; 2],
}
```

`Vector` 为当前 Rapier 的 `Vector`。地图方法：`small_world() -> Self`、`build_geometry(&self, spacing: f32) -> MapGeometry`、`edge_length(&self, edge: usize) -> f32`、`edge_point(&self, edge: usize, along: f32, offset: f32) -> Vector`、`project_edge(&self, edge: usize, position: Vector) -> EdgeProjection`、`signed_distance(&self, position: Vector) -> f32`、`nearest_edge(&self, position: Vector) -> usize`。

`circulation::graph`：`GraphNetwork::new(&MapDefinition, viscosity: f32) -> Self`；`solve(&self, pressure_drop: f32) -> GraphFlow`。`GraphFlow` 公开 `pressures: Vec<f32>`、`rates: Vec<f32>`、`summary: circulation::Flow`；summary 的总流量为泵段流量，mean_speed 为泵段截面均速。`sample_velocity(position: Vector, map: &MapDefinition, flow: &GraphFlow) -> Vector` 是后续局部流体优化的替换边界。

`Simulation` 保留原 API，增加 `new_world(Config) -> Self`、`world_map() -> Option<&MapDefinition>`、`world_geometry() -> Option<&MapGeometry>`。`geometry()` 仍返回当前场景墙轮廓。`Snapshot` 增加 `branch_flows: Vec<f32>`（实验场为空）；其余现有字段保留。`reset()` 保留所选场景、参数、暂停状态。

bridge 增加 `use_world()`（当前 Config 创建世界）、`world_geometry() -> Dictionary`：`walls` 为轮廓数组，`floor` 为平铺三角形顶点的 PackedVector2Array，`bounds` 为两个角点，`landmarks` 为 `{name, position: Vector2, size: Vector2}` 字典数组。snapshot 加 `branch_flows` 数组；原几何接口不变。

## 任务 1 地图与共享几何

**worker 所有权：** 新增 `crates/simulation/src/map.rs` 和必要的 `map/geometry.rs`；模块内少量测试。禁止修改 lib/runtime/view/circulation/bridge。

- [x] 实现固定地图接口，制作少于 16 节点的心脏、分流、两支路、汇流、回流样板。路径转角平滑采样，明确图节点连接；地图超过视窗，管宽容纳现有机器人和细胞。
- [x] 以 0.2 网格步长提取通道并集；marching squares 的墙与地面共享交点及鞍点判定。无内部墙，初始化一次生成。
- [x] 增加一个图连接处的共享几何关键检查，覆盖管内通行点不被内部墙截断、填充与边界基本一致。检查调用真实几何接口，不断言源码或穷举全部实现分支。
- [x] 运行本模块相关测试，报告轮廓和三角形规模；未接线导致暂不能编译则及时报告主 agent。

## 任务 2 图流量与局部采样

**worker 所有权：** `crates/simulation/src/circulation.rs`（只加子模块导出，保留实验 API）、新增 `circulation/graph.rs` 及其模块内测试。禁止修改地图或 runtime。

- [x] 按节点收支、泵边压升、参考压力构造 f64 约化系统并 Cholesky 求单位解；每条边保留正阻力。运行时只按压差缩放。
- [x] 实现按边抛物线采样与仅在邻接路口附近的平滑混合；同一条边不因折线细分而重复计权。以共享地图距离限制外墙附近流速。
- [x] 一个独立手算的串并联回路验证压力/流量、节点守恒与缩放；一个采样行为检查确认无泵压时静止、普通段和路口连续性。控制测试数量。
- [x] 运行本模块相关测试，报告近似限制与实际结果。

## 任务 3 接入与世界画面

**worker 所有权：** `crates/godot_bridge/src/lib.rs`、新增 `godot/world.tscn`、`world.gd`、世界绘制/HUD 所需小脚本、`godot/project.godot`。不重写原 `lab.gd`，不改 simulation。

- [x] 按固定接口扩展 bridge；保留原实验入口。
- [x] 世界场景只有一个固定步调用点；静态地面可用 MeshInstance2D/ArrayMesh，避免每帧重建大量三角形。动态细胞、机器人和流向沿用简单样式。
- [x] Camera2D 跟随快照机器人，CanvasLayer 承载 HUD、暂停/单步/重置/参数及流向与碰撞开关。展示三个地标与分支流量，保持文案具体简洁。
- [x] 默认启动世界场景；现有 lab.tscn 仍能独立启动。主 agent 在集成后集中执行 bridge 编译与两个场景加载，worker 不运行交互测试。

## 任务 4 主 agent 集成与集中验证

**所有权：** `lib.rs`、`runtime.rs`、`cell.rs`、`view.rs`、`tests/experiment.rs`、现有/新增真实模拟探针、`scripts/check.sh`、README 与本轮文档。

- [x] 原实验/世界通过私有场景枚举组合各自地图与循环；统一 `velocity_at` 采样，细胞 advect 接受速度采样闭包，不再绑定单血管。Rapier、驱动、固定时钟仅保留一套。
- [x] 世界从共享墙、出生点和按路径长度分布的 72 个细胞构造；机器人附近留空间；快照按各边采样箭头并传实际流量。
- [x] 保留已有实验测试；增加世界暂停/重置的必要断言，尽量复用已有用例。用真实模拟探针记录支路经过、面积、越界和求解耗时，发现具体问题再针对修正。
- [x] 一次集中 `bash scripts/check.sh` 覆盖格式、Rust、扩展和两场景加载；图形自然运行截图查看。必要时只重跑受影响检查。
- [x] 进行一次整体独立审阅，修复核心问题；同步 README、设计实际落地和本计划的测量/限制。保持工作区未提交状态。

## 接口自查

| 交接 | 生产与消费 | 结论 |
| --- | --- | --- |
| 地图 → 循环 | MapDefinition、edge_length、project_edge、signed_distance | 同一公开类型与方法，无文件所有权重叠 |
| 地图/循环 → runtime | 静态几何、出生、GraphFlow 与速度采样 | 主 agent 统一接线 |
| runtime/view → bridge | new_world、world_map/world_geometry、branch_flows | 字段已固定，保持旧接口 |
| bridge → Godot | world_geometry 字典、snapshot 与现有控制 | 单个 worker 负责两端 |
| 各任务内部 | 文件、接口、算法、关键检查 | 与已批准设计一致；不扩展测试或未批准系统 |

## 执行记录

- 2026-10-22：设计与推荐算法经用户确认；用户要求保留后续流体及其他子系统优化空间。
- 工作区沿用 feat/engine-foundation 的未提交原型；不新建工作树、不自动提交。三块子任务明确文件责任，主 agent 集成后集中验证。


## 实际交付与验证

2026-10-22 本批完成。三个实现 worker 使用 `gpt-6-sol`、`medium`；主 agent 完成共用 runtime、真实模拟探针、文档和集中验证。工作区保留未提交状态。

- 世界样板为 11 节点、12 段：心脏、两个并联器官通路、汇流与回流。默认 72 个细胞全部详细运行。
- 网格步长 0.2，生成 3346 个墙线段与 102110 个地面三角形；两者共用离散交点。Godot 静态地面一次建立 Mesh，墙线一次转换后批量绘制。
- `MapDefinition`、图压力求解、局部速度采样、速度驱动、runtime 和表现数据各自保留明确边界。细胞读取速度闭包，原实验场与世界共用同一物理推进逻辑。
- 原实验默认构造器仍可用；世界入口为 `Simulation::new_world` / `BloodLab.use_world`。重置保持场景、参数和暂停语义。
- 主场景改为 `world.tscn`，Camera2D 跟随机器人，HUD 使用 CanvasLayer。流量列表限高滚动，底部说明通过截图发现并修正锚点；原 `lab.tscn` 可独立运行。

### 实测与性能修正

首次未优化的模拟模块在新地图上测得约 91.4 ms/步；将 simulation 开发构建设为 opt-level 2 后，10 秒探针平均 16.76 ms。临时采样探针测得局部速度查询约 11.72 ms/步，定位到重复的完整投影。随后将只需距离的查询改为平方距离比较，最近边每条边仅查询一次，未更换流速算法或增加依赖。临时性能脚本已移除。

最终命令：

```bash
cargo run --locked -p simulation --example circulation_probe -- 72 60 world
```

| 项目 | 结果 |
| --- | --- |
| 模拟规模 | 72 个 20 粒子软体，11 节点、12 段，所有区域详细运行 |
| 模拟时长 | 60.0 秒，3600 固定步 |
| 场景初始化 | 161.8 ms，包含几何、网络与物理构造 |
| 探针墙钟时间 | 54.76 秒，包含快照与统计 |
| 逐细胞累计移动距离 最小 / 中位 / 最大 | 8.066 / 126.765 / 165.439 模拟单位 |
| 分流口进入上 / 下支路 | 36 / 2 次下游边转换记录 |
| 上 / 下支路进入回流干道 | 25 / 16 次下游边转换记录，包含初始已在支路中的细胞 |
| 有软体接触的固定步 | 3460 / 3600 |
| 最大面积偏差 | 3.05% |
| 最大包围框长宽比 | 1.859 |
| 最大粒子越壁距离 | 0.0000，按固定步采样与隐式边界比较 |
| 求解 平均 / p95 / 最大 | 13.220 / 18.088 / 49.096 ms |

环境沿用 Linux x86_64、Xeon Gold 6330；dev 构建，simulation 与依赖均 opt-level 2。求解时间包括流速驱动和 Rapier，不包括 Godot 绘制，也不是目标玩家设备的帧率。

下游转换记录由实际细胞中心的所在边变化统计，用于观察通行，不是流体体积计量。两支路流量相等，但细胞分配明显不均；部分细胞停留较久，当前近似场不能保证均匀交通或所有细胞完成整圈。这是后续局部流体优化的具体观察依据。本轮不据此加入双向流体或全局堵塞模型。

### 集中检查与画面

- `cargo test --locked -p simulation`：修改前已有基线通过。
- `bash scripts/check.sh`：最终 Rust 格式、workspace 检查、4 个模块检查与 5 个集成检查、扩展构建、Godot 导入及世界/原实验两个场景各 180 帧加载均通过。新物理检查验证真实细胞进入两支路并通过汇流，复用已有暂停检查覆盖世界重置。
- 图形场景在 `DISPLAY=:2` 自然运行后保存截图；无自动按键、按钮或控件操作。全景图只改变观察相机，用于看清地图结构。修正底部帮助锚点后重新截图，未为布局变动重复运行整套 Rust 检查。
- 截图：`docs/images/world-small-map.png`、`docs/images/world-small-map-overview.png`。图形环境为 Mesa llvmpipe，出现不支持调整 VSync 的驱动提示；场景没有脚本或扩展错误。
- 一次整体独立只读审阅未发现当前内置样板的阻断性问题。审阅指出节点坐标和边中心线端点存在两份表达；当前构造器从 nodes 派生首尾，满足一致性。后续增加地图制作入口时需维持这一约束，本轮不为尚未存在的导入路径增加验证框架。

人工尚需体验镜头跟随、逆行、选路与细胞交通。地图编辑、实际器官内容、战斗、区域装卸、远处生态、大地图性能和更好的局部流体仍按后续任务推进。
