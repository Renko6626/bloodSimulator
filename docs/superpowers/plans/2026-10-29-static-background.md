# Godot 静态血管纹理缓存实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. 默认由当前 agent 顺序实施，不根据本计划自动派生子 agent。

**Goal:** 地图加载时绘制一次血管底板、普通管壁与地标，正常帧复用一张 GPU 纹理，同时保持现有物理、动态对象和调试入口。

**Architecture:** Godot 表现层新增背景资源组件和静态 Painter，通过 SubViewport.UPDATE_ONCE 生成缓存，Sprite2D 直接复用 ViewportTexture。Rust 与 GDExtension 继续提供原来的静态字典，World 只重绘动态和启用的调试内容。

**Tech Stack:** 现有 Godot 4.7.1 / GDScript / GL compatibility；无新依赖，Rust/wgpu 不改。

**Spec:** [静态背景设计与调研](../specs/2026-10-29-static-background-design.md)。

## Global Constraints

- 用户后续明确安排“很好，继续，准备开干”，现已实施 Task1/2 并完成本地验证；实卡复测另行安排，不自动提交、推送或合并。
- 保留工作树已有 profiling 文档和原始证据，不覆盖或丢弃其他修改。
- SCALE=24；8px 留边；当前支持单张缓存不超过4096×4096；背景颜色、字体、普通墙线抗锯齿沿用原效果。
- 只调整 Godot 表现；不改静态字典、动态 snapshot、碰撞、LBM、Rapier、默认地图及参数。
- 使用不透明 LDR 缓存、独立 World2D、UPDATE_ONCE；不使用 CPU 图像回读、每帧缓存生成、启动等待渲染信号或新恢复机制。
- 不调整动态细胞和机器人画法；flow/collision 与 HUD 保留，调试模式成本单独解释。
- 默认只截图看 UI，不自动点击按钮、填表或发送键鼠交互；新增永久测试须遵循 AGENTS.md 的具体缺口要求，本计划不默认新增。

## Review Focus

1. 验证场地标在 geometry.bounds 外：Task 1 用原地图/字体核对完整内容矩形，Task 3 截图包含地标。
2. 负坐标和小数镜头移动：Task 1 明确整数缓存原点；Task 3 比较默认地图/验证场的相同画面，检查漂移与接缝。
3. 离屏世界或层级错误：Task 1 独立 World2D；Task 2 移除主 viewport 旧静态节点；Task 3 截图确认无重复对象、黑块或遮挡。
4. headless/关闭 render loop：Task 1 提前跳过纹理分配且无 await；Task 3 用既有命令确认模拟实际推进及退出，不能只看 exit0。
5. 误把静态缓存收益当作完整 World 控制收益：Task 2 保持动态画法；Task 3 成对比较及独立记录冷启动、纹理尺寸/显存、背景更新和完整帧间隔。

---

### Task 1: 静态 Painter 与一次性纹理组件

**Files:**
- Create: `godot/world_background_painter.gd`、`godot/world_background.gd`。
- 对应 `.gd.uid` 由 import 生成。

**Interfaces:**
- Consumes: 现有 `world_geometry()` 字典 `{walls, floor, bounds, landmarks}`、`SCALE`、`ThemeDB.fallback_font`。
- Produces: Painter 的 `configure(geometry: Dictionary, pixels_per_unit: float, font: Font) -> void`、`get_content_rect() -> Rect2`；背景组件的 `configure(geometry: Dictionary, pixels_per_unit: float, font: Font) -> String`，空字符串为成功。

- [x] **准备：冻结同机基线。**
  在改动 Godot 文件前保存当前 `godot/` 的脚本/场景与 release bridge SHA，作为 Task 3 未缓存对照。benchmark 参数沿用现有 `--lbm-benchmark=move --lbm-benchmark-warmup=120 --lbm-benchmark-frames=600 --lbm-benchmark-output=...`；两侧加载同一份 release `.so`，禁止拿 debug/release 或跨机结果相减。
- [x] **Step 1: 实现 Painter 的静态数据准备与内容范围。**
  一次性构造原底板 ArrayMesh、缩放墙线和地标。按 spec 合并 geometry bounds、地标框和真实字体文字范围；保持背景/底板/框和名称/墙的绘制顺序与颜色。
- [x] **Step 2: 实现背景组件配置。**
  有渲染时计算整数原点、尺寸与8px边距，超4096返回具体错误；创建独立 World2D 的 SubViewport、Painter 和一个 z=-10 的非居中 Sprite。使用 spec 的 LDR、不透明、3D/MSAA关闭、CLEAR_MODE_ONCE、UPDATE_ONCE 配置。
- [x] **Step 3: 绑定纹理和生命周期。**
  在一次 deferred `_bind_texture() -> void` 中直接赋 ViewportTexture；Painter.position=-origin，Sprite.position=origin，scale=1、linear filter。资源随背景组件释放，不增加等待循环、状态机或 ImageTexture 转换。
- [x] **Step 4: 保留无绘制运行。**
  `DisplayServer.get_name()=="headless"` 或 `not RenderingServer.is_render_loop_enabled()` 时，configure 成功返回但不创建 viewport/mesh/sprite；不阻断模拟、snapshot、正常退出。
- [x] **Step 5: 检查脚本与布局元数据。**
  在已有 GDExtension 导入缓存下，对两个新脚本运行 `godot --headless --path godot --script res://world_background_painter.gd --check-only` 和 `... res://world_background.gd --check-only`，应 exit0。核对两张当前地图分别约2244×1217、3780×3206，确认地标在范围内；不为此新增复制布局算法的永久测试。

### Task 2: 接入 World，移除重复静态绘制

**Files:**
- Modify: `godot/world.gd`、`godot/world.tscn`。

**Interfaces:**
- Consumes: Task 1 的背景 `configure(...) -> String`。
- Produces: World 保留唯一模拟 tick 和原 benchmark；默认画面使用静态 Sprite；动态 `_draw` 不再提交普通墙/底板/地标。

- [x] **Step 1: 场景接入。**
  在 world.tscn 添加带脚本的 `StaticBackground` Node2D，移除根 viewport 中旧的 `Floor`、`LandmarkBackgrounds` 节点。Camera2D 与 HUDLayer 保留。
- [x] **Step 2: 初始化接线。**
  World 创建 simulation、取静态 geometry 后配置背景；返回非空错误时复用 `_startup_error`。删除旧 `_build_floor/_build_landmark_backgrounds` 与相应节点引用；仅保留 collision 调试所需 wall_segments。不要每帧或 reset 再调用背景 configure。
- [x] **Step 3: 缩小动态 `_draw`。**
  删除普通管壁与地标绘图；保留 flow 箭头、黄色墙碰撞线、全部细胞填充/轮廓/调试线、机器人。不开 collision 时没有任何墙段进入动态 `_draw`；开关不重烘焙。
- [x] **Step 4: 核对模拟入口与相机。**
  `_physics_process` 的 tick 次数、snapshot、输入回放、HUD 更新、Camera.position、暂停/单步/reset/调参保持原语义；不将等待烘焙写入这条路径。默认 world、local-stokes 和 validation 均使用同一背景组件；lab 不改。
- [x] **Step 5: 运行相关现有检查。**
  `godot --headless --path godot --script res://world.gd --check-only` 应 exit0；整体改动完成后集中运行一次 `bash scripts/check.sh`。既知首次 editor import 134 若重现，保留日志并区分 cached import/实际运行，不在本任务修复引擎退出问题。

### Task 3: 集中验证、测量与交付记录

**Files:**
- Update: 本计划、`docs/STATUS.md`。
- Create: 实施后的 `docs/research/2026-10-29-static-background/README.md` 和必要原始日志/截图/测量数据；不是本轮计划阶段的新实验。
- 不新增云端 CI 工作流、独立端到端工程或永久行为测试。

- [x] **Step 1: 正常启动与无渲染检查。**
  `godot --headless --path godot --fixed-fps 60 --quit-after 180`、同命令增加 `--disable-render-loop`、以及原 lab 入口应正常推进并退出；结合已有 snapshot/启动输出或临时只读诊断确认模拟有推进。headless 明确不作为纹理正确性证据。
- [x] **Step 2: 有绘制截图（人工操控未验收，见实施记录）。**
  默认地图和验证场各看一张自然运行截图，检查地标、负坐标对齐、管壁/底板边缘、文字颜色、细胞前后层级。仅截图不自动交互；碰撞/流向开关、reset 和手感由用户人工检查，报告未人工验证项。若使用既有显式 benchmark 取得同帧截图，不发送键鼠输入。
- [ ] **Step 3: 有界实卡成对测量。**
  另行安排硬件后，沿用现有 release 库、Fine/72细胞/同一 MoveInput 前缀，运行未缓存和缓存两例各120预热＋600测量步；只使用既有 benchmark，不新建采样框架。保留完整帧间隔、Rust step、LBM等待、draw calls、GPU时钟/状态、输入/负载记录。若时钟/吞吐随时长明显漂移，才将这一对延长到2400测量步，不机械追加大矩阵。
- [ ] **Step 4: 单独核对缓存。**
  用一次性只读诊断记录实际纹理 size、CPU资源配置时间、首帧墙钟和稳定帧静态内容不再进入渲染图元统计（后端自动禁用，Node getter 仍为 UPDATE_ONCE），说明每帧静态来源不再渲染；CPU configure 耗时不冒充 GPU 烘焙时间，首帧诊断可用非阻塞的一次性 frame_post_draw 日志，不引入启动等待；冷启动不并入 steady-state均值。纹理颜色预算10.42/46.23 MiB不冒充总显存，其他 render target/mesh 的实际占用另报。若实际帧仍慢，记录动态细胞的剩余成本，不顺带改变细胞抗锯齿或物理。
- [x] **Step 5: 保存证据与说明限制。**
  图形比较、物理记录量对比只声称实际检查的量；不将单个背景 Sprite 称为全场1 draw call。记录实际命令、失败和未验证项；云测量按项目既有规则先取回核验后 stop、确认 stopped/exited，保留磁盘。仅在用户另行安排时提交/推送。

## 本轮计划检查

- [x] 选定路线与现有静态字典能够对应，无新增 Rust/bridge 字段。
- [x] API 已查官方文档，并在本机 Godot 4.7.1 核对元数据。
- [x] 矩形范围包含验证场的外置地标与文字，尺寸/颜色纹理预算已经计算。
- [x] 无渲染不等待、层级、负坐标、reset、调试显示和资源所有权均有明确处理。
- [x] 验证沿用既有检查和回放；截图/人工检查与自动检查边界明确，不一律新增测试。
- [x] 不把 no-World/no-AA 控制混同静态背景缓存，未承诺未测量的帧率。
- [x] Task1/2实施、本地现有检查、两场景同帧截图及独立审阅完成。
- [ ] 实卡性能、冷启动/总显存、人工操控验收尚未完成，不能报告实卡加速或60FPS。

## 本地实施结果

2026-10-29，源码接入完成，已有checks通过。默认场景图元171112→8730，验证场371742→8434；动态画法保持，drawcalls仍为401/399。两张地图缓存大小与计划一致。Node级update_mode getter不反映backend自动关闭，已据引擎源码修正核验口径。详情与实际命令见 [实施记录](../../research/2026-10-29-static-background/README.md)。


用户现已安排阶段性文档与提交推送，见[阶段交接](../../research/2026-10-29-performance-checkpoint.md)。本地证据已归档；本计划仍未勾选的实卡/冷启动/总显存/人工验收项保持待验证，不因归档自动执行。
