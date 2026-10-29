# 游戏帧成本定位

2026-10-29，基线 `41057e3`。本轮为性能诊断，不修改正式物理、桥接接口或渲染后端。诊断修改仅通过 [instrument.py](instrument.py) 应用于提取的源码副本。

## 现有证据重新对齐

来源为 [上一轮 RTX 3060 原始结果](../2026-10-29-lbm-gameplay/hardware-rtx3060/results/)。此前完整 3600 测量帧中，移动案例 Rust 独立整步平均 10.077 ms：LBM 4.321 ms、Rapier 4.148 ms、耦合 0.233 ms、prepare 0.261 ms，剩余约 1.114 ms 未分项。LBM 中的 readback 等待为 2.272 ms，已包含在 LBM 阶段内。

为与短控制公平比较，以下统一使用移动案例预热 120 帧后前 600 个测量步；帧间隔取两端均在测量期的 599 项。全部为平均墙钟毫秒，不能将本表与完整 3600 帧统计混用。

| 阶段 | 独立 Rust | Godot 正常 GL | Godot headless | GL 关闭 render loop |
| --- | ---: | ---: | ---: | ---: |
| Rust 整步 | 7.962 | 29.045 | 16.835 | 17.578 |
| prepare | 0.237 | 0.488 | 0.493 | 0.508 |
| 机械耦合 | 0.220 | 0.504 | 0.510 | 0.514 |
| LBM，含等待 | 2.788 | 20.526 | 8.016 | 8.497 |
| 其中 readback 等待 | 1.070 | 17.079 | 4.424 | 4.747 |
| Rapier | 3.701 | 5.322 | 5.619 | 5.873 |
| 整步内未分项 | 1.017 | 2.204 | 2.197 | 2.187 |
| tick 调用减 Rust 整步 | — | 0.006 | 0.007 | 0.007 |
| snapshot/桥接/UI 刷新 | 0.727¹ | 1.000 | 1.599 | 1.621 |
| process 帧间隔 | — | 65.687 | 29.088 | 38.440 |
| 间隔扣除当前 tick 与刷新² | — | 35.617 | 10.640 | 19.226 |

¹ 独立 Rust 仅 `snapshot()`，不含 Godot 封装、validation quality 与 UI；不能直接比较两列作桥接净成本。

² 用间隔结束时对应的物理步逐行扣除。剩余包含前次绘制、引擎调度、日志及其他工作，不是纯渲染/GPU 时间。

计算见 [summarize-baseline.py](summarize-baseline.py) 与 [matched-baseline.json](matched-baseline.json)。原 baseline `.so` 和 probe 的 SHA256 均与上一轮封存二进制相同。

已能确定：

- GDScript→Rust tick 的额外调用成本约 6–7 μs；没有证据支持把约 60 ms 差距归为数据桥接。
- Godot 内 Rust 自身已经变慢，不能仅分析 tick 返回后的绘制。相同前缀 GL 与 headless 的 Rust step 差约 12.21 ms，readback 等待差约 12.66 ms；这一关联尚不能区分 GPU 算子变慢、队列排队、驱动同步与 CPU 调度。
- 无绘制也比独立 probe 慢；Rapier、耦合与整步未分项均变化。绘制不是现有证据中唯一需要追踪的部分。
- 显式进程级关闭 vblank 的旧控制未改善结果；当前不能将问题归结为单一 vsync 开关。

## 本轮诊断方法

保留未加计时的同一 baseline；另做同场景、同输入、同物理的诊断副本。保持 72 个十二节点细胞、Fine 192²、每游戏帧 14 格子步、2 机械子步及原 GL 后端。完整运行安排见 [run-hardware.sh](run-hardware.sh)。

- `PROFILE_STEP`：已有 LBM upload、encoding、submit、wait、decode 分项，连同 prepare/耦合/Rapier；这些是嵌套分项，不能重复求和。
- `PROFILE_INNER`：线程池内 `step_inner`。与外部 step 墙钟差值包含线程池交接、计时、日志等，不能直接当纯调度等待。
- `PROFILE_ADVECTION`：机器人驱动与全细胞背景拖曳，覆盖原先整步剩余成本中的主要循环。
- `PROFILE_SNAPSHOT`：Rust snapshot 与返回前封装时间；完整桥接返回、UI 和 CSV 由 GDScript 独立计时补齐。
- `.gd.csv`：`_draw` 发绘图命令的脚本墙钟、完整 physics callback、CSV 构造/写入、快照桥接、HUD；`_draw` 不代表 GPU 绘制执行。process 时记录的是前一次 `_draw`，边界帧须排除。
- `BLOOD_GPU_PROFILE=1`：对原有 compute pass 写入 timestamp pair，随已有 field/population readback resolve/map，不增加 queue submit 或 device poll。记录所有 pass 耗时总和及首尾跨度；跨度可含 pass 间停顿，pass 总时间也不能解释为只含 ALU 的计算量。启用时间戳仍可能扰动性能，须对照不开时间戳的诊断运行。
- `no-world` 仅停止 World `_draw` 发命令，保留背景和底板；`no-hud` 停止 HUD 更新并隐藏 HUD。均是实验开关，不是正式优化。
- 可选 [wait-trace.c](wait-trace.c)：单独 LD_PRELOAD 控制记录 ≥1 ms 的若干 libc 等待的墙钟和线程 CPU 时间，按 TID 分析，空闲工作线程等待不等于帧阻塞。它不能覆盖所有驱动内部等待，主对照不启用它。

`BLOOD_ALLOW_SOFTWARE=1` 仅在诊断副本中允许软件适配器，用于少量功能检查。软件运行时间不能作为 RTX 3060 性能证据。GPU timestamp 仅覆盖 LBM compute pass，不覆盖 GL 渲染、提交前等待、传输或 CPU 密度统计。

## 本轮实卡结论

本轮新实例 `58212504`：RTX 3060 12 GB，NVIDIA 570.195.03，Xeon E5-2673 v3，24 个逻辑 CPU，容器配额 23.04 核。仍用 Godot 4.7.1、GL compatibility、1280×800、四线程 Rapier。`glxinfo` 和 Godot 日志均确认 NVIDIA 硬件渲染。本轮驱动/CPU 与上一轮不同，因此先在本机复现 baseline；不以跨机器比值宣称优化效果。

**主要成本定位在 World 绘图命令路径及其伴随的 LBM 执行/完成等待变化。tick 调用桥接只有微秒量级；单纯隐藏 HUD 不解决问题。** 关闭世界绘图后仍保留全部细胞、机器人、流体、快照、HUD、底板和背景；它是定位控制，不是可交付的游戏画面。

下表除最后两行外，均为 120 预热＋600 测量步、599 帧间隔；最后两行为独立延长的 2400 测量步。平均/p95 均为 ms。诊断副本启用日志或时间戳，会有测量干预，不能将每行当作无干预生产性能。

| 控制 | Rust step 平均 | 帧间隔平均 / p95 | draw calls |
| --- | ---: | ---: | ---: |
| 原始独立 Rust baseline | 7.806 | — | — |
| 原始 Godot baseline | 27.915 | 61.862 / 68.084 | 412 |
| 诊断日志，无 GPU 时间戳 | 25.622 | 57.832 / 67.389 | 412 |
| 正常绘制，GPU 时间戳 | 28.828 | 63.733 / 69.860 | 412 |
| 正常绘制复测，时间戳＋时钟采样 | 30.025 | 66.750 / 69.920 | 412 |
| 关闭 World 中线条抗锯齿 | 19.516 | 36.798 / 39.816 | 258 |
| 隐藏 HUD 并停止其更新 | 31.102 | 67.489 / 72.867 | 378 |
| 跳过 World `_draw` | 8.568 | 11.787 / 13.294 | 36 |
| headless | 18.036 | 29.234 / 31.592 | 0 |
| GL 保留，关闭 render loop | 18.286 | 38.501 / 40.308 | 0 |
| 跳过 World `_draw`，2400 步 | 10.443 | 13.826 / 20.174 | 36 |
| 独立 Rust 诊断，2400 步 | 9.622 | — | — |

对应 [完整统计](profile-summary.json)、[短程/延长原始记录](hardware-rtx3060/verified/results/) 与 [成对延长记录比较](long-state-comparison.json)。原始 baseline 的 .so/probe SHA256 与上轮封存一致。日志版本/时间戳版本相对 baseline 有数 ms 波动，故报告保留三者与正常复测，不把其中最好的一次替代生产成绩。上表未给出完整游戏达 60 FPS 的验收结论：关闭抗锯齿仍约 37 ms；隐藏世界的延长控制 p95 仍约 20 ms。

### 1. Rust 的约 10 ms 从哪里来

本轮独立 Rust 延长诊断 2400 步，整步平均 9.622 ms、p95 11.237 ms：

| 分项 | 平均 ms | 口径 |
| --- | ---: | --- |
| LBM | 4.175 | 含约 2.016 ms readback 等待 |
| Rapier | 3.701 | 两个机械子步合计 |
| 窗口 prepare | 0.260 | 含迁移帧，延长案例实际迁移 4 次 |
| 有限阻力耦合 | 0.240 | marker 交换 |
| 其余 | 约 1.246 | 包含下列背景驱动、线程交接与诊断开销 |

新增背景驱动计时平均约 1.000 ms；线程池内外墙钟差平均约 0.072 ms。这些包含于整步，不能再次相加。CPU 密度统计和其他缓存工作包含于 LBM，GPU pass 总时间平均约 2.039 ms；readback 等待不等于纯数据传输。

因此，后续 Rust 优化应分别看 Rapier 接触/软体、LBM 完成等待与帧末 CPU 工作；现有证据不支持将线程池交接当作主要热点，也不支持只看 2 ms 纯流体旧预算。

### 2. World 绘制为何优先处理

- 玩家位置、活跃/附近细胞、接触见证、密度、面积和迁移次数等 12 个已记录量，在 baseline、全部主要短程控制及关闭抗锯齿案例的 720 步中逐行 f32 相同；syscall 控制仅比较其 300 步前缀。两组延长案例的对应 2520 步也全部相同。比较仅覆盖已记录量，未声称所有内部状态逐位一致。
- 当前 `_draw` 每帧重新发墙线、地标、72 个细胞的两层多边形、轮廓及机器人命令。正常诊断中 `_draw` 脚本平均约 5.5–6.3 ms；同一阶段并不包括之后的全部 RenderServer/GL 成本。抗锯齿控制仅改 `_draw` 中八处 draw API 的布尔参数，物理完全相同；实际 draw calls 从 412 降至 258，整帧和 LBM 等待同时改善。
- 只隐藏 HUD 后仍慢；HUD 更新本身约 0.08 ms，快照桥接约 0.8–1.1 ms，CSV 构造/写入约 0.2–0.4 ms，均不足以单独解释几十 ms。
- **headless 和 `--disable-render-loop` 仍调用 `_draw` 并构造几何命令。** 实测脚本平均分别约 6.13/10.76 ms。它们不等同于跳过绘图命令构造，之前“无绘制仍慢”的部分成本由此获得直接证据。

补充源码几何核对：直接调用当前 `MapDefinition::build_geometry(0.2)`，验证场 7 条血管得到 7740 条管壁短线段、223964 个底板三角形、1 个地标；默认 small_world 为 3346 条短线、102110 个底板三角形、3 个地标。底板网格初始化一次并复用，管壁的 `draw_multiline` 则和动态细胞处于同一 `_draw`，每步 `queue_redraw()` 后重新发出全部线段命令，源码没有先按镜头范围筛选这些线段或细胞。不能将几何段数等同 draw call 数，也不能因底板三角形多就判定它是主因：跳过 World 的控制仍保留底板。当前 `_draw` 墙钟还包含 draw API 内部的几何构造，不能全归为 GDScript 解释器成本。

关闭抗锯齿是高价值局部控制，尚未修改正式画面；不能把约 30 ms 的整帧差值全部命名为某一个 draw API 的 CPU 时间，也不能将禁用世界显示作为性能修复。

### 3. GPU 时钟与等待也确实改变

时间戳记录的是同一批约 33 个计算 pass 的 GPU 时间范围。独立 Rust 短程平均 pass 合计约 1.266 ms；正常绘制约 7.921 ms，正常复测约 8.400 ms；关闭抗锯齿约 5.180 ms。正常复测 readback 等待约 17.077 ms，关闭抗锯齿约 6.289 ms。两类时间不能互相相加或简单相减解释为另一类独占成本。

[200 ms 间隔的只读时钟采样](clock-summary.json)显示：

- 正常复测 234 项采样中 192 项为 P8，核心频率中位数 210 MHz。
- 关闭抗锯齿的核心频率中位数约 307 MHz，仍多数处于 P8。
- 无 World 的延长案例以 P5 为主，核心频率中位数约 705 MHz；独立 Rust 延长案例中位数约 600 MHz。
- 所有案例启动附近都经历过约 1807 MHz 的高频段。因此追加了延长控制：无 World 的均值由短程约 11.8 ms 变为延长约 13.8 ms，不能以启动时最快前缀宣称长期 60 FPS。

这支持低功耗状态是性能差距的一部分，也解释了为何短程纯流体预算不能直接外推。尚未独立控制 GPU 时钟、测 GL 的 GPU timeline 或采集驱动栈，**不能将电源管理、GL/Vulkan 排队与上下文调度唯一分摊**。本轮没有锁频、改驱动/内核设置或替换渲染后端。

### 4. 系统调用提供的限制证据

单独 120 预热＋180 测量步的 `strace -f -tt -T -yy` 控制完成，跟踪下帧间隔约 95.7 ms，明显扰动正常性能，只用于定位等待类别。[汇总](syscalls-summary.json)中主线程累计 futex 等待约 11.78 s、ioctl 约 0.67 s，没有主线程 nanosleep/clock_nanosleep；长 sleep 来自后台线程，不能按累计时间算到每帧。主线程等待异步 Rust 物理时也会落在 futex 中，不能把 11.78 s 全部当成 GL 等待。详细 syscall 日志仅存于压缩结果包，避免重复存放 22 MB 文本。

可选 libc interposer 只得到空记录，不能据此声称没有等待；strace 证明其没有覆盖这些内部调用。本轮未使用容器不支持的 perf/eBPF，也没有扩展 kernel 权限。

## 建议的下一步

优先在保持现有技术路线和物理负载下，评估 World 线条抗锯齿与细胞轮廓的绘制方式、静态几何重复发命令及批次成本。先做局部画面改动的成对验证，再决定是否需要改 Godot 节点/渲染结构。Rust 则优先看 Rapier 与 LBM 帧末/同步，线程池交接优先级较低。

这些是本轮 profile 得出的后续建议，不代表已授权更换后端、增加异步调度机制、调整系统模块边界或开启下一轮云实验。

## 复现与验证

实际完整命令、结果和未验证项见 [验证记录](validation.md)。

本轮诊断副本的 [完整源码](instrumented-source.tar.gz)、[源码差异](hardware-rtx3060/source.diff)、[二进制 SHA](binary-manifest.sha256)与三组 runner 均已保留。先用 `git archive` 创建副本再执行 instrument.py；脚本拒绝在带 `.git` 的目录修改正式工作树。

```bash
python3 summarize-baseline.py ../2026-10-29-lbm-gameplay/hardware-rtx3060/results matched-baseline.json
python3 analyze-profile.py hardware-rtx3060/verified/results profile-summary.json
python3 analyze-clocks.py hardware-rtx3060/verified/results .
# syscall 大日志按需从包中提取，再运行 analyze-syscalls.py。
tar -xzf hardware-rtx3060/results-bundle.tar.gz -C /tmp results/syscalls.log
python3 analyze-syscalls.py /tmp/results/syscalls.log syscalls-summary.json
```

- 诊断 bridge/probe 的 `cargo build --offline --locked --release ... --features lbm-gpu` 通过；Godot `--script res://world.gd --check-only` 通过；`cc -shared -fPIC -O2 -Wall -Wextra ... -ldl -pthread` 和三个 runner 的 `bash -n` 通过。
- 软件适配器各 3 步 probe/Godot 的 smoke 均 COMPLETE/exit0，仅作诊断工具功能检查。
- 实卡主矩阵、聚焦控制、延长控制和实际游戏退出均为 0；各场景 COMPLETE。没有按钮点击或表单交互验证，也没有新增永久测试。
- 首次无缓存 editor import 仍为 134，沿用既知独立异常记录；后续实际游戏和脚本检查为 0，不宣称修复导入问题。
- 结果包 SHA 与远端一致，所有实验结果文件逐项 SHA 通过；只有持续运行的 Xorg 在生成清单后继续写入 `Xorg.99-fixed.log`，该显示日志的清单时间点与打包内容不同，已明确记录于 [取回核验](hardware-rtx3060/retrieval-verification.json)。原始包保留未篡改。
- 实例 `58212504` 已确认 `stopped/exited`，见 [停机回执](hardware-rtx3060/final-stop-status.json)。磁盘保留，仍可能有存储费用；未销毁实例。
