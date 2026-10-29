# Godot 静态血管纹理缓存设计与调研

2026-10-29。用户已选择方案 1“静态纹理缓存”，本轮要求调研和做计划。用户随后明确安排开始实施；已在 `feat/godot-static-background` 接入并完成本地检查/截图，实卡对照尚未执行，未启动云实例。实现与证据见 [记录](../../research/2026-10-29-static-background/README.md)。现有性能证据见 [profile](../../research/2026-10-29-gameplay-profile/README.md)。

## 目标与当前成本

血管底板、普通管壁、地标位置/底色/边框/名称均为静态内容，应在地图加载时绘制一次，正常帧复用一张背景纹理。红细胞、机器人、血流箭头、碰撞辅助线及 HUD 保持动态。

当前 `world.gd` 每次 `_refresh()` 都调用 `queue_redraw()`，同一 `_draw()` 重发地标、7740 条验证场管壁短线、细胞与机器人绘图命令。底板 ArrayMesh 已经复用，但仍每帧绘制约 22.4 万个三角形。此次缓存同时移除正常帧中静态命令重建和底板网格的反复绘制；启动时的几何生成和一次栅格化仍然存在。

性能控制表明 World 路径值得优化，但“跳过 World”也跳过了细胞/机器人。该控制的 13.8 ms **不能**作为仅缓存血管后的性能预测。关闭抗锯齿对照也同时改变细胞轮廓，本轮保持动态绘制原样以单独测静态缓存收益。

## 路线比较与选择

| 路线 | 收益/代价 | 决定 |
| --- | --- | --- |
| SubViewport 绘制一次，复用其纹理 | 沿用当前几何和外观，无新依赖；增加一张地图分辨率纹理；放大清晰度受缓存分辨率限制 | 用户已选 |
| 简化闭合轮廓＋少量纹理网格 | 有利于缩放和减少启动几何，但需处理孔洞/分叉及显示数据组织 | 本轮不实施 |
| 只把管壁移到静态 Node2D | 可缓存命令，底板与大量线段仍参与每帧实际渲染 | 不足以完整实现所选路线 |

保留 Rust/wgpu、Godot GL compatibility 和 GDExtension 接口。`MapDefinition`、`MapGeometry`、碰撞墙、LBM 几何/参数和动态 snapshot 不改。缓存只消费现有 `world_geometry()` 返回的 walls/floor/bounds/landmarks；几何来源仍是当前权威地图。

## Godot API 调研

- [SubViewport.UPDATE_ONCE](https://docs.godotengine.org/en/stable/classes/class_subviewport.html#enum-subviewport-updatemode) 在绘制一次后切换为 `UPDATE_DISABLED`，适合静态缓存。
- [ViewportTexture](https://docs.godotengine.org/en/stable/classes/class_viewporttexture.html) 可直接赋给 Sprite2D；不需要 `get_image()`→ImageTexture 的 CPU 回读和再次上传。该转换成本较高，且高分辨率 viewport 的创建本身可能有启动卡顿。
- [Viewport.world_2d / find_world_2d](https://docs.godotengine.org/en/stable/classes/class_viewport.html#class-viewport-property-world-2d) 支持独立 2D 世界。烘焙 viewport 显式设置 `World2D.new()`，防止把游戏动态对象或缓存 Sprite 自身纳入离屏画面。
- [RenderingServer.render_loop_enabled](https://docs.godotengine.org/en/stable/classes/class_renderingserver.html#class-renderingserver-property-render-loop-enabled) 可区分禁用渲染循环的控制。headless 使用 dummy rendering，不是图片正确性的验证环境。
- `ViewportTexture` 在场景 root 未 ready 时可能返回不正确的数据。用一次 deferred 绑定在场景 ready 后设置 Sprite.texture；不让模拟启动等待 `frame_post_draw`。渲染依赖由 Godot 处理，首次显示需在实现截图检查中验证。

本机 Godot 4.7.1 已通过 ClassDB 核对 UPDATE_ONCE=1、CLEAR_MODE_ONCE=2、`is_render_loop_enabled()`；临时只读查询还用实际 fallback Font 测量了地标文字范围，输出见 [recon.txt](../../research/2026-10-29-static-background/recon.txt)。查询没有渲染血管图片，也没有证明实际 GPU 缓存行为。

## 文件与职责

| 文件 | 责任 |
| --- | --- |
| `godot/world_background.gd`，新增 | 拥有一个 SubViewport 和显示 Sprite；配置一次性烘焙、坐标对齐和资源生命周期 |
| `godot/world_background_painter.gd`，新增 | 消费现有静态字典，准备 Mesh/墙线/地标；报告绘制范围；只画静态场景 |
| `godot/world.gd` | 保留模拟/输入/benchmark/HUD 调用；配置背景；`_draw` 只画动态内容和开启的调试内容 |
| `godot/world.tscn` | 用 `StaticBackground` 替换主 viewport 中的 `Floor`、`LandmarkBackgrounds`；保留 Camera/HUD |

两个脚本都局限于 Godot 表现层，不建立通用缓存框架。对应 `.gd.uid` 由 Godot import 生成。`lab.gd/lab.tscn` 不调整。

### 组件接口

- `world_background.gd`: `configure(geometry: Dictionary, pixels_per_unit: float, font: Font) -> String`。空字符串表示配置成功；具体尺寸错误返回文字，由 World 复用 `_startup_error()`。每个 World 实例调用一次；不是每帧入口。
- `world_background_painter.gd`: `configure(geometry: Dictionary, pixels_per_unit: float, font: Font) -> void` 和 `get_content_rect() -> Rect2`。矩形坐标使用已经乘 SCALE 的世界像素坐标。
- `world_background.gd`: 私有 `_bind_texture() -> void`，只 deferred 调用一次；没有每帧 `_process`、重试队列或恢复状态机。

## 绘制与资源生命周期

1. World 仍按现有入口创建模拟、取一次静态几何。无渲染时直接完成背景配置，不分配大纹理/静态 Mesh；physics、snapshot 和 benchmark 继续推进。
2. 有渲染时，Painter 准备静态数据；背景组件根据其完整内容范围计算整数纹理矩形，再创建独立 World2D 的 SubViewport。
3. 使用 `disable_3d=true`、`use_hdr_2d=false`、`msaa_2d=MSAA_DISABLED`、`transparent_bg=false`、`CLEAR_MODE_ONCE`、`UPDATE_ONCE`。绘制一次后由 Godot 自行禁用更新。
4. 背景使用与主 viewport 相同的项目默认清屏色，输出不透明纹理。当前场景下面只有清屏底色，这样可避免透明边缘的预乘 alpha 额外处理；不改变项目全局清屏色或混合设置。
5. Painter 的局部位置为 `-cache_rect.position`。显示 Sprite 使用 `centered=false`、`position=cache_rect.position`、scale=(1,1)、linear texture filter，位于动态对象下方的 z=-10。
6. deferred 绑定直接使用 `viewport.get_texture()`，不复制图像。保留 SubViewport、Painter 和其静态资源到场景退出，避免增加一次性回收回调；它们在 steady state 不重绘。
7. World reset、暂停、单步、压力/速度/拖曳调参，以及 flow/collision 开关不改变静态地图，不重建缓存。未来切换地图通过重建 World 场景获得新缓存；本轮不加全局多地图缓存或失效管理。

### 外观与层级

Painter 严格保留当前顺序：地标背景 → 血管底板 → 地标框/文字 → 普通管壁。保留原颜色、文字字号和普通管壁抗锯齿，首先只改变绘制频率。

动态 World `_draw` 保留：flow 箭头、开启时的黄色墙碰撞线、72 细胞两层填充/轮廓/可选碰撞线、机器人圆与轮廓。黄色墙调试线仅在 collision 开启时绘制，不烘入背景；开关不触发背景重烘焙。调试模式允许有额外成本，不能用其结果代表正常游戏。

静态层独立后，World 的 `queue_redraw()` 不会重建 Painter 的命令；Camera2D 只变换背景 Sprite 与动态对象，不参与重烘焙。

## 缓存范围、尺寸和显存

**不能只使用 `geometry.bounds`。** 验证场的血管最小 y≈20.6，但地标中心 y=13、半高2.5；只截血管矩形会丢失地标。

Painter 的内容矩形合并以下范围：

1. `geometry.bounds` 对应的整个静态几何范围，乘 `SCALE=24`。
2. 每个地标背景/框的矩形。
3. 原绘制公式得到的文字 baseline，加 Font 的文字宽、ascent 和 height 得到的文字矩形。

最后 `origin=floor(rect.position)-Vector2(8,8)`，`end=ceil(rect.end)+Vector2(8,8)`，`size=Vector2i(end-origin)`。8px 留边覆盖当前线宽/抗锯齿边缘，原点必须取整以避免额外半像素偏移。

| 地图 | 缓存像素原点 | 预计尺寸 | RGBA8 颜色纹理 |
| --- | --- | --- | ---: |
| 默认 small_world | (-901,-421) | 2244×1217 | 10.42 MiB |
| LBM 分叉验证场 | (-1290,244) | 3780×3206 | 46.23 MiB |

这是颜色纹理的字节计算，不是总显存承诺；还要计入驱动 render target、字体、保留的静态 Mesh 等。保持当前缩放分辨率，不生成 mipmap、不做超采样，不自动按窗口缩放重烘焙。当前两图单边都小于4096；本轮支持范围明确为单张不超过4096×4096，超限给出具体错误，不静默降低分辨率或自动引入分块系统。

## 验收与范围

- 静态内容对齐、地标不被裁切，颜色/管壁边缘与旧版相符；背景下方不出现黑块或重复世界对象。
- 稳定帧 SubViewport 渲染后端已停止更新（Node 属性仍可能保留 UPDATE_ONCE），缓存来源无反复渲染；普通 reset/参数/显示开关不要求重新建立背景。
- 动态细胞、机器人、镜头、HUD、flow/collision 行为保持。图像默认仅截图检查，键鼠手感留用户人工验收。
- headless 与 `--disable-render-loop` 不等待渲染信号、不分配缓存纹理，原有检查继续实际推进模拟；它们不证明缓存显示正确。
- 同机同物理的未缓存/缓存对照，分别记录冷启动和 steady state。常驻背景应成为一个 Sprite 绘制，整帧仍包括细胞/HUD，**不**要求全场 draw calls 变成1。
- 若静态缓存后细胞抗锯齿仍占主要成本，报告剩余瓶颈，后续单独安排；本轮不连带更换细胞形状/材质、流体调度或后端，不承诺60FPS。
- 不引入新依赖、桥接字段、Rust 模块边界、渲染后端、地图拓扑或云端服务。本次调研/计划不启动云实验。
