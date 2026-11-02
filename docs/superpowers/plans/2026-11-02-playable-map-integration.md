# 可玩路网实装计划

> For agentic workers: superpowers:subagent-driven-development；用户已选sol medium实施、sol high审阅。2026-11-02用户以“开干”确认接入方案与本轮范围，开始执行。

**Goal:** 候选A成为可实际走图、独立选择的新地图几何预览。
**Architecture:** 复用MapDefinition/VesselEdge/ChamberGeometry、单Rapier和固定区块；新图与旧验证场分开，有限的双预览类型保证零状态和reset正确，不引入场景注册框架。
**Tech Stack:** 现有Rust/Rapier/Godot/SVG工具，无新依赖。
**Spec:** [接入方案](../specs/2026-11-02-playable-map-integration-design.md)，空间依据[候选A](../specs/2026-11-02-playable-vessel-network-design.md)。

## Global Constraints

- 候选A分区、环路、多入口、原尺寸四腔及平移保留；24/12/6/3，两个3单位窄联络，无固定1/4/16/64均流；整体跨区尺度保留，不以省时收紧。
- 四类心脏接口、左右隔墙及两外网串联逻辑保留；A/B/C仅预留区域，不新增房间/任务/设备/敌人/实体层。
- 新`MapDefinition::organ_world`、`Simulation::new_organ_preview`、`BloodLab::use_organ_preview`、`--organ-preview`及`map_render --map organ`；默认/旧入口和旧验证图保持。
- 预览类型仅旧Heart/新Organ两项；`is_geometry_preview`两者为true，旧`is_heart_preview`只旧场为true；step/参数/reset零pressure/pulse/红白数量，reset保留地图与暂停。
- 固定16单位块、地图/实体5×5、LBM3×3、zoom2和单物理世界不改；流场未启用，不在新图自动启动LBM。
- 只用定宽分段和现有几何并集修正转弯/接口；无半径模型、动态地图格式、生成器、路由框架、虚拟边、假立交、新求解器/依赖。
- 统一图求解只给单位驱动诊断，本轮不确定最终Q/压差，不逐边覆盖速度；旧Q48/烘焙域/资产不继承为新图数据。
- 不进行烘焙、性能/云实验、自动UI流程或长时间全图巡游；不自动push/merge。已有检查集中，局部变化局部补验。
- 工作位置为宿主管理的`/data/sunyunbo/.codex/worktrees/4aad/bloodSimulator`，执行前确认分支与干净状态；Cargo统一使用`CARGO_TARGET_DIR=/data/sunyunbo/www/bloodSimulator/target`。各worker不独占代码库，不回退他人修改，不自行扩大架构或再委派。

## Review Focus

- 脚本共享坐标未必是MapDefinition图节点：Task1同时检查真实图连通和SDF路口，不只看图形。
- 宽管端帽、接口和共享路口角度可造成新开口/封口：Task1心腔隔离与典型净空，Task3局部实际移动。
- 新预览误用旧bool会在参数/reset后回旧图或恢复压力：Task2真实双模式测试和桥接统一守卫。
- 肺下口近路及同侧环路可能有低/反向流量：Task1/3如实输出单位图结果，不虚构校准完成或强行均流。
- 首尾安全不代表整条导航安全：Task3先连续路段SDF检查、再逐帧即时守卫，不重现上一阶段漏弯点的长回放失败。

### Task 1: 候选A真实地图与单位图诊断

**Ownership:** 新`map/organ.rs`及其私有测试；`map.rs`声明/入口；`map/heart.rs`仅抽出四腔原始轮廓的小型私有复用，不改旧外管。研究草图保持历史来源，不成为运行依赖。
**Interfaces:** `MapDefinition::organ_world()->Self`；局部手工道路构造辅助私有。若需显式诊断路线，提供与现有heart路线同风格的`organ_preview_route()->Vec<(&'static str,[f32;2])>`，仅包含短程代表路线，转角均取真实中心线。

- [x] 手工迁移候选A的道路与真实心腔，拆分有意路口/宽度接续，普通转角留折线；保留整体位置和32条逻辑道路，因拆边/过渡产生的实际边数如实报告，不设假相等约束。
- [x] 用短定宽连接与现有圆角几何改善接口，允许局部控制点修正。保留肺下口近路和A/B/C占位，不把分区底色画成流体空间。
- [x] 少量生产MapDefinition/SDF/GraphNetwork测试检查连接、隔离、典型净空与守恒；单位默认黏度求解有限且如实保留正反/低流边，不填新参考驱动。
- [x] 聚焦新地图与原heart检查、fmt/diff，自审本地提交；完整检查留Task3。

### Task 2: 独立预览入口与reset/零状态

**Ownership:** `runtime.rs`、原`runtime/heart_preview.rs`可按职责重命名`runtime/geometry_preview.rs`；`godot_bridge/src/lib.rs`、`godot/world.gd`、`world_hud.gd`、`examples/map_render.rs`及必要使用文档。
**Interfaces:** 新`new_organ_preview`与`is_geometry_preview`；旧`new_heart_preview/is_heart_preview`保留各自语义。私有双预览类型；桥接`use_organ_preview`；HUD对应标题与共享零状态控件设置。

- [x] 接入新工厂/模式标记，所有普通构造无预览类型，reset重建对应地图并保留暂停；一步/参数突变后仍零流/零细胞，旧heart语义不变。
- [x] Godot解析新入口/互斥规则并沿用预览禁用不兼容模式；新旧标题明确，WASD/speed/pause/N/R/C保留，压力/drag/flow/F按两种预览共同防护。
- [x] `map_render --map organ`使用新工厂，原默认heart/small-world和旧诊断不改；少量双模式/重置真实测试，新工具入口聚焦检查。
- [x] 聚焦受影响检查、fmt/diff，自审本地提交，不自动UI测试或启动长回放。

### Task 3: 有限实际通行、源图与集中交付

**Ownership:** 新显式`examples/organ_preview_probe.rs`、`docs/research/2026-11-02-playable-map-integration/`、STATUS/本计划/必要工具说明；地图/运行文件缺陷报告给对应任务owner，不并行越权编辑。
**Interfaces:** Task1真实地图/短路线、Task2新预览工厂及静态导出入口；标准MoveInput和现有物理，不新增公开测试控制接口。

- [x] 在正常出生点走一条覆盖心室、肺中央路口与窄联络的短路线；体侧路口/另一窄联络通过明确起点的局部独立场景初始化检查。实际走动无中途瞬移；地方案例不声称全图普通出生路径已验收。
- [x] 先检查连续计划路段的SDF净空，再实际推进；即时检查有限位置/清楚的身体净空、零状态、现场块数，失败停在具体路段。旧radius.85和已有移动/时间步不变，不因失败松守卫。导航只为诊断，不加运行时寻路。
- [x] 导出实际地图全景和心肺局部、单位图各段数据/分流说明与来源，比较草图和实装偏移；注明非心腔实际流场，未校准最终驱动。
- [x] 集中运行`bash scripts/check.sh`（含默认套件，不另重复）、`cargo test --locked -p simulation --features lbm-gpu`、`cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`；新旧预览headless加载，最多静态截图。只运行明确有界的新增探针，不重跑旧211点长回放；修正只补受影响检查。
- [x] 更新文档/结果/来源并本地提交，独立任务与最终整分支审阅由主agent安排，记录未验收项；工作树保留，不推送合并。
