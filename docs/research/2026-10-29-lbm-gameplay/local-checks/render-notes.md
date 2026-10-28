# Godot GL 回放性能差距：限定只读检查

2026-10-29。仅读现有源码、启动脚本及已结束的 smoke CSV；未改代码/参数，未启动 Godot、GPU 或新实验。主代理提供的完整实测差距仍是观测值，下列按代码事实与待验证推断区分。

## 先排除重复推进与额外流体读取

- `world.gd:148–161` 每个 `_physics_process` 只有一次 `simulation.tick` 和一次 `_refresh`。`_process` 只写 render CSV；`_draw` 只绘图，均不再次 tick/snapshot。benchmark 模式挡住键盘 single-step/reset 路径。未见第二套物理推进。
- 既有 smoke：420 个 physics CSV 行、420 个 process CSV 行，相邻 process 行 `physics_frame` 增量全部为 1（419/419）；每步均 14 lattice steps。readback 次数只有 3/4，分别符合普通帧与迁移多一次 field。不能把约 68 ms process 间隔解释成一帧跑了两次 30 ms simulation。
- `BloodLab.snapshot` 使用共享 CPU cache；validation quality、cell/arrow 封装没有额外 `LbmWindow.field()`。当前 CSV 没显示 Godot 多做一次 raw/field read 的证据。

## 计时口径中实际容易漏算的部分

| 列 | 实际边界与影响 |
|---|---|
| `step_wall_ms` | `state.solve_ms`，由 Rust `runtime.rs:408–410` 在 `threading::install(step_inner)` 外计时；和 probe 的同一物理步口径接近，含等待但不含 snapshot/UI/render。不是 GPU kernel 时间。 |
| `tick_wall_ms` | GDScript 调用 Rust tick 的 wall；包含上列及桥接调用，不能与 step 再相加。 |
| `snapshot_refresh_ms` | `world.gd:154–156,231–235`：Rust snapshot、validation quality、Godot Variant/PackedArray 封装、camera 赋值、HUD 文本更新、queue_redraw 的合计；名字不等于纯 snapshot。真正 `_draw` 不在此区间。 |
| `physics_callback_before_csv_ms` | 传给 `_benchmark_row` 的参数在进入函数前已经算完，故 **不含整个 `_benchmark_row`**：再次遍历 72 个 cell 的中心/速度、数组/字符串构造、physics CSV 写入、每 120 帧 flush、guard、截图协程。不能将此列视作完整 physics callback。 |
| `process_callback_before_csv_ms` | `_process` 数组最后一个字段在 `store_csv_line` 执行前求值；不含实际 CSV 写入，也没有 `_draw`、RenderServer、swap/present。数值小不能证明渲染回调总成本小。 |
| `render_interval_ms` | 两次 `_process` 开始时刻之差，涵盖其间 simulation、snapshot/UI、两份 CSV、draw/driver/engine 调度及任何等待。约 68 ms 是这一路径的吞吐间隔，不能把 `68−30` 全部叫 GL 绘制耗时，更不是 GPU timestamp 或显示延迟。 |

`_process` 每次还查询 `RenderingServer.get_rendering_info`、显示后端和渲染方法（world.gd:172–173）；它不是绘制本身。若内部存在跨线程统计同步，耗时应落在 process_callback_before_csv 的求值区间；项目源码无法证明它有 driver fence，也未见显式 force_sync/glFinish 调用。

各阶段 p95 不能直接相加。每 120 帧 flush 可能制造周期性长尾；它在上述 callback 参数计时之外。截图只在 warmup 一次 `frame_post_draw` 后读纹理/存 PNG（`world.gd:376–379`），无法解释持续整个测量区间的差距。

## 现有画面每帧确实重建什么

- `_refresh` 每个 physics tick 无条件 `queue_redraw`，全世界共用一个 World Node2D 的 `_draw`（`world.gd:231–289`）。静态墙线、landmark 框/文字也跟随动态细胞一起重新发出绘图指令。未见 `_draw` 里再次 queue_redraw 的自循环；不同 UI 事件也没有在 benchmark 中反复触发。
- 72 个 cell 全部进入 GDScript 循环，无手工视口/局部窗口筛选。每个至少两次填充多边形、一次带抗锯齿的 outline，并逐帧创建 scaled、closed、inner 点数组。即使后期 active_cells 降为 0，画面仍遍历并发出全部 72 个 cell 的指令。因此局部机械负载减轻，不代表绘制负载同步消失。412 drawcalls 与这条细分绘制路径相容，但 source 调用数不能直接等同最终 drawcall，也不能由 drawcall 数推算毫秒。
- 默认 `show_flow=false`、`show_collision=false`，无输入回放去打开它们；不能把全部箭头和第二套 collision outline 算进当前显示负载。另一方面，`Simulation::snapshot` 仍生成全图 flow arrows，bridge 仍封装它们，即使不显示；这是 CPU snapshot/封装开销，主要落在 snapshot_refresh，不应拿来解释 Rust 内的 LBM wait。
- HUD `update_state` 每个 tick 更新多行 stats 和每条分支 flow 文本（`world_hud.gd:95–102`）。脉动值与 solve_ms 经常变化，意味着持续文本/布局工作；它发生于 snapshot_refresh，后续 UI 绘制位于 engine render 区间。此函数不回写 slider、不发 parameters_changed，未见压力设置递归或多次 simulation.tick。
- Floor mesh 和背景 Polygon2D 只在 `_ready` 构建一次，不是逐帧重建大网格。Camera2D 开了 smoothing（world.tscn:15–17），每次 refresh 设置位置；这会影响相机处理/可见画面，但没有证据表明它能造成额外 LBM dispatch。

## 额外等待的具体候选与仍不能下的结论

1. Godot GL rendering 与 wgpu 的独立 compute/readback 使用同一 RTX3060，进程中未见 GL↔wgpu 共享纹理/显式同步接口。已有代码没有在 tick 内显式等待 GL；但上一帧 GL 工作、driver/context 调度或 GPU 电源/时钟状态影响下一轮 wgpu 完成等待，是与当前结构相符的**候选**，尚非已证根因。
2. `gpu.rs:356–367,582–599` 的 `readback_wait_ms` 从 map_async 前计到 poll/wait+channel 完成为止，包含此前排队的碰撞/重建/拷贝完成及主线程调度延迟。18.6 ms 不能解读为 PCIe 拷贝 2.5 MB 需要 18.6 ms，或纯 FD/normalization kernel 变慢；也不能把它在 LBM22 ms 外再加一遍。
3. Rust simulation 内 Rapier wall 同时变慢，也提示差距并非已被“多边形 GPU 填充”单独解释。Godot loop/driver 与已有四线程物理池共享 CPU，wall 计时会吃到调度与等待；只读源码无法归因具体线程/驱动锁。
4. 实际脚本已经显式 `--fixed-fps 60 --disable-vsync --audio-driver Dummy`，项目 60 Hz physics、GL compatibility、1280×800；未设额外 frame cap、render-thread 模型、低处理器模式，也未见脚本 sleep/await 每帧限速。Xorg 配置 `UseDisplayDevice=None`，使用离屏 screen。不能仅凭参数断言 driver/present 完全不等待，也不能因 P8/25% 采样断言 GPU 性能已饱和或算法 kernel 很慢。
5. 本次启动为项目运行，非 `--editor`；fresh editor 导入退出的独立 134 问题不能直接作为 gameplay 慢的解释。运行日志确认 NVIDIA GL 和 wgpu hardware，不是 llvmpipe 混入。

主代理已安排的 headless 与 `--disable-render-loop` 对照正好区分显示/GL context/常规 render loop 影响。解释时同时核对 14 steps、3 readbacks（迁移帧4）、实际轨迹及 CPU snapshot 成本：headless 改变显示后端整体；禁 render-loop 与原 GL 窗口的差异也不能自动叫“纯 GPU 绘制耗时”。当前无需据此追加渲染架构或参数修改。
