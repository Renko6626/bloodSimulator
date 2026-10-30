# 固定区块加载与实体卸载实施计划

> **For agentic workers:** 使用superpowers:executing-plans，主agent按顺序实施，末尾一次独立只读审阅。用户已授权实装，随后另行安排整理提交并合并到本地main；本轮不推送或启动云实例。

**Goal:** 固定16单位区块；LBM 3×3，地图与实体5×5，支持真正的现场资源装卸与白细胞休眠。

**Architecture:** 全局地图/流量/数量库存，唯一Rapier世界，附近块派生几何；Cell身体互斥Active/Dormant，Godot只消费增量显示数据。

**Tech Stack:** 现有Rust、Rapier 0.36.1、Godot/GDExtension和可选LBM；无新依赖。

**Spec:** [已确认设计](../specs/2026-10-30-world-streaming-design.md)。以本版16单位规则为准，旧32单位草案作废。

## Global Constraints

- 共享区块16，现场半径2块；LBM维持3×3窗口和原子步。边界身体按完整AABB保留相关墙，禁止再引入独立96/128或7×7常驻范围。
- 新入口`--stream-world`，zoom2；旧入口及历史负载保持。
- 红细胞库存每0.2模拟秒输运；跨块重分区，现场构造避开实际画面。
- 活体休眠冻结AI，尸体计龄继续，所有白细胞记录计入8只上限。拥堵不移位/换id/补血。
- 不增加后台队列、存档、ECS、票据框架、空间索引或远场个体运动；云实验与性能细化仍暂停。
- 验证按完整改动集中执行，UI仅截图。新增测试只覆盖实际交接与接缝，不复制算法或建立大型验证项目。

## Review Focus

- 负坐标/采样接缝不封口或重复墙。
- Dormant恢复不改变原始面积/材料约束，身份和健康保留。
- AI不更新休眠对象，尸体/暂停/reset/人数上限正确。
- 查询刷新在新墙使用前完成；跨边缘活跃身体需要的墙不被卸载。
- Godot新增/移除及reset不保留过期块或全部历史纹理。

### Task 1：共享网格和局部几何

**Files:** fluid/lib.rs、fluid-gpu/lbm/mod.rs、simulation/map.rs、map/geometry.rs。

**Interfaces:** `WORLD_CHUNK_SIZE`；`ChunkId::at/bounds/neighborhood/scene_bounds`；`MapDefinition::build_chunk_geometry`输出core/paint局部几何。

- [x] 共享基础16，负坐标向下取整，5×5范围固定在玩家所在块。
- [x] 用已有几何算法生成指定块及2采样格留边，不先建全图。
- [x] 增加接缝真实几何用例，检查墙长度及无人工封口；相关测试通过。

### Task 2：实体身体表示

**Files:** cell.rs、runtime/gameplay.rs、runtime/immunity.rs、runtime.rs及原测试访问。

**Interfaces:** `CellBody::Active/Dormant`；`Cell::active_body/hibernate/restore`；单份身份/生命/行为记录。

- [x] 保存粒子形态/速度后删除身体，恢复原静止约束并设置保存状态。
- [x] 驱动、攻击、感知、marker和轮廓快照仅使用Active；休眠不占物理世界。
- [x] 变形、身份和健康恢复用例通过；旧行为测试保留有效断言。

### Task 3：Rust现场交接与碰撞装卸

**Files:** 新world_streaming.rs、runtime/streaming.rs；runtime.rs、cell_population.rs、immunity.rs。

**Interfaces:** `new_streamed_world`；`WorldStreaming`局部块缓存；`update_streaming`；`StreamingSnapshot`。

- [x] 流式world无全图MapGeometry，固定5×5和跨边界身体所需墙局部加载/移除。
- [x] 红细胞窗口按块中心重分区；现场范围及保护由共享网格/镜头确定。
- [x] 白细胞休眠恢复、尸体计龄、查询刷新、同地图reset接入。
- [x] 综合往返释放与休眠计时用例通过；数量/免疫/尸体原用例通过。

### Task 4：Godot增量背景与摄像机

**Files:** bridge/lib.rs、world.gd、world_background.gd、world_background_painter.gd、world_hud.gd、lab.gd。

**Interfaces:** `configure_stream_world`；`sync_world_view`返回reset/added/removed；背景`apply_chunk_delta/clear_chunks`。

- [x] 显式入口、实际镜头范围与zoom2；静态数据仅新增时传输。
- [x] 每块一次SubViewport烘焙，384像素核心、8像素留边、地标裁切与销毁。
- [x] 当前场景截图与Godot导入通过，无自动键鼠；HUD显示活跃/休眠和当前块数。

### Task 5：LBM和旧路径兼容

**Files:** runtime/streaming.rs、runtime.rs、fluid/streaming_window.rs、bridge和现有lbm_gameplay_probe。

**Interfaces:** `new_streamed_lbm_world/new_streamed_lbm_validation`；原LBM窗口仍独立读全局地图。

- [x] 新LBM路径不先生成全图几何；reset保留流式/validation及原种群选择。
- [x] 全workspace all-targets feature编译通过。
- [x] feature测试与一次软件GPU移动短测覆盖至少一次地图/流体移窗；旧探针不改变默认负载。

### Task 6：规模、文档与审阅

**Files:** 新examples/streaming_probe.rs；README、STATUS、研究记录与本计划。

- [x] 300/1000/3000跨度地图资源短回放，零细胞，峰值25块，往返墙数恢复；不称FPS验收。
- [x] 集中检查默认/feature构建测试、旧world/lab及流式场景加载；截图只覆盖必要画面。
- [x] 一次独立只读审阅；仅修实际核心问题并按影响层验证，不追加理论机制或测试矩阵。
- [x] 写实际证据、限制与64×64候选草案；提交推送另按用户安排。
