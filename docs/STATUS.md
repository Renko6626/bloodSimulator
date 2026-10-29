# 当前状态与待办

更新：2026-10-30。此前阶段已归档至 `main`；当前实施分支：`feat/godot-static-background`。

**性能细化暂停，会话到此结束。** 用户要求之后有必要再继续。已将LBM/Rapier优化空间判断、新版外层3.37ms未细分及后续候选写入[交接记录](research/2026-10-30-performance-checkpoint.md#暂停前的判断与后续候选)；这些不构成自动profiling、GPU归约实施或云实验授权。

此前FD/LSQ、均值控制和可玩验证场已归档至main；静态血管缓存、视野剔除和细胞模拟窗口已在d0dfa07归档。用户本次要求继续阶段性提交推送，纳入步长诊断、端到端实卡证据、帧内分解，以及细胞Mesh/LBM合批实现，保存于当前分支。见[最新交接](research/2026-10-30-performance-checkpoint.md)。优化后的实卡复测、移窗prepare尖峰、首次editor导入异常等继续保留，不自动开始下一轮优化或云实验。

## 阶段状态

用户随后要求先推送25e29e4再做实卡，现已完成同轮RTX3060往返对照：旧版/新版各两次3600步，合并整帧均值14.578→11.973ms（-17.9%），p95 18.092→14.684ms，≤16.7ms间隔比例68.17%→99.31%，仍有28ms尾帧。四个主测各5次移窗、生成3/退休36，记录轨迹字段一致。补充分项时频率/主机成本变化，不能精确分摊Mesh/HUD/LBM收益。120个原始文件已核对，58263433再次stopped/exited；未改产品代码。见[实卡复测](research/2026-10-30-cell-render-lbm-batching/hardware-rtx3060/README.md)。

用户已确认并完成细胞绘图/LBM合批：可见细胞复用双层填色ArrayMesh、原抗锯齿轮廓保留；HUD统计10Hz。普通Fine帧14步/33dispatch/3读回不变，submit20→8；420步软件GPU真实回放含一次移窗通过。默认场景同帧792000世界像素一致，本机600帧绘图CPU1.496→1.348ms、draw calls147→126，但软件渲染整帧24.51→24.93ms，没有端到端提速证据。极薄/深凹时内层与轮廓的层叠差别保留为Minor；本地阶段当时未做RTX复测，后续见上条。默认检查、LBM相关检查、GL几何用例和独立审阅完成；本次纳入阶段提交，优化实施未启动云实例。见[实现与证据](research/2026-10-30-cell-render-lbm-batching/README.md)。

用户安排的进一步分解已完成，生产代码未改。补充组15.179ms整帧中，Rust9.146ms、物理回调内其余1.063ms、World动态绘图1.786ms、物理后引擎/延迟处理1.562ms、render跨度1.455ms，其余0.167ms；边界时序闭合。LBM4.936ms中三次正常读回等待约1.486/0.241/0.408ms；独立GPU时间戳组compute合计3.203ms，与等待存在重叠。连续组均值15–21ms且GPU频率波动，不能混组相减或称稳定60FPS；旧101ms未复现、未定因。83个原始文件已取回核对，58263433再次stopped/exited。见[帧内分解](research/2026-10-29-game-profile-detail/README.md)。没有自动实施下一轮优化。

新版本端到端实卡复测已完成：同机旧版LBM前600步均值/p95为48.928/56.028ms，新版Full为17.407/18.808ms；新版60秒LBM Full/Local均值约15.17/15.13ms、p95约18.45/19.79ms，仍未稳定达到60FPS，Local有未归因101ms尖峰。默认解析流地图Full/Local固定回放p95约8.82/6.61ms。LBM Local实际3次生成/36次退休、5次移窗，数量与运行守卫通过。结果取回核对后实例58263433已stopped/exited，未启动后续优化。见[端到端报告](research/2026-10-29-game-e2e/README.md)。

用户随后批准LBM格子速度预算0.05→0.07的诊断试验。实际14→10步，72细胞固定/移动各720步及无机械反馈控制通过运行守卫，但12秒末速度场相对差异约40%–48%，无反馈仍约41%；不能视为轻微精度损失。生产默认仍0.05/14步，仅诊断副本改动，无新云实验。见[试验、原始数据与限制](research/2026-10-29-lbm-timestep-relaxation/README.md)。

细胞窗口方案已获确认并实现：`--local-cells`使用72×72近场激活、80×80完整AABB退休，窗外按图边连续区间每0.2模拟秒输运数量；默认全量模式保留。实际CPU 60秒回放中实体28–55个，生成66/退休68，总量误差2.84e-14；本机solve均值6.335→3.755ms、p95 7.552→4.684ms，仍有长尾。这是解析流/CPU证据，不能外推成RTX/LBM或70ms整帧问题已解决。LBM软件短测、Godot截图与现有检查通过；硬件、LBM移窗与细胞交接联合长测和人工手感尚未验收。见[实现与证据](research/2026-10-29-cell-population-window/README.md)、[设计](superpowers/specs/2026-10-29-cell-population-window-design.md)和[计划](superpowers/plans/2026-10-29-cell-population-window.md)。

Godot细胞视野剔除已按后续指令接入：真实轮廓AABB与实际相机矩形相交才生成绘图命令，8px余量；默认同帧draw calls401→146，世界截图像素一致。

用户选定的静态血管纹理缓存已在 `feat/godot-static-background` 接入；现有检查与两场景同帧截图通过，图元数量明显下降，动态细胞画法未改。RTX实卡性能复测、冷启动/总显存和人工操控尚未验收，未启动新云实验。见 [实施证据](research/2026-10-29-static-background/README.md)、[设计](superpowers/specs/2026-10-29-static-background-design.md) 和 [计划](superpowers/plans/2026-10-29-static-background.md)。

**此前性能定位已完成，以下数字保留测量时的原始实现口径。** 新 RTX3060 实例 `58212504` 上，同负载正常绘制复测平均帧间隔 66.75 ms，关闭 World 线条抗锯齿的诊断副本为 36.80 ms；跳过 World 绘图的延长 2400 步控制平均 13.83 ms、p95 20.17 ms。tick 桥接约 6–7 μs，headless 仍构造 `_draw` 几何；正常绘制存在 P8/210 MHz 与 LBM 等待增大的关联，尚不能唯一分摊驱动/上下文/电源管理。不能把隐藏画面控制视作游戏达到 60 FPS。完整源码、原始数据、计时干预及限制见 [本轮 profile](research/2026-10-29-gameplay-profile/README.md)。实例已取回结果并确认 stopped/exited；下一轮优化或云实验需新安排。

**已完成可玩 LBM 分叉验证场和实卡机械耦合/移动回放；完整游戏帧率仍未达标。** `--local-lbm --lbm-validation` 使用真实机器人与 72 个十二节点细胞，固定与移动各完成 120 预热＋3600 测量帧。玩家实际往返并完成 5 次移窗，运行守卫无失败。Rust 完整模拟步 p95 为 12.48/12.15 ms，移窗峰值 24.86 ms；Godot 默认 GL 绘制在该 RTX3060/Xorg 云环境中帧间隔 p95 为 71.40/70.39 ms。无绘制及驱动同步对照已完成，下一步需定位 Godot 绘制/引擎/流体同步共存成本，不能将纯流体预算当作游戏 FPS。见[完整实测](research/2026-10-29-lbm-gameplay/README.md)。

首次 headless editor 导入退出存在已复现的 134 异常，根因未明；cached import 和实际游戏运行/退出为 0。所有结果已收回核验，实例 58133143 已 stopped/exited。

**用户已批准将 LSQ-FD 给定速度边界和帧末平均密度控制接入现有可选 LBM 模块，本轮已在接入分支实现。** 沿用 D2Q9 MRT＋Guo、48×48 移动窗口、图网络边界与现有 Rapier 耦合；`lbm-gpu` 默认关闭，默认游戏仍用解析流。算法、物理代价及适用范围见 [LBM 物理文档](physics/lbm-fluid.md)，接入与检查见 [实施记录](superpowers/plans/2026-10-29-lbm-integration.md)。

固定 `fork_a Fine` 的 RTX 3060 成对 10000 帧实验完成：控制后平均密度残差最大约 2.12e-10，纯流体预算平均 1.90 ms、p95 2.10 ms。它仍持续补偿质量，也改变总流体动量；不能作为自动守恒、完整游戏性能或机械耦合精度证明。见 [预算与长测](research/2026-10-29-lbm-budget-longrun/README.md)。默认 small_world 开口、边界直接切分叉、近切向/欠分辨开口仍不支持；当前没有首发地图验收结论。

此前固定速度重建的 10 个实卡失败案例和 [Sol high 因果调查](research/2026-10-28-lbm-streaming-window/hardware-rtx3060-20261028/independent-investigation.md)保留为原算法负例。本轮沿用精确速度/体积流量契约，不改成固定密度出口；此前周期边界研究也不构成正式开口窗口验收。

RTX 3060 上，Re=100、完整 192²／256²、72 细胞的流体加耦合 p95 为 2.44／3.93 ms，整步为 6.77／8.83 ms，不含渲染。低 Re 高黏度工况的成本限制保留。详见[结果报告](research/2026-10-27-ns-lbm/lbm-results.md)。

上一阶段 GPU Stokes 性能优化仍暂停；其约 46–184 ms／次的回放是另一种算法的历史结果，不代表本次 LBM。旧记录见[Stokes 阶段交接](research/2026-10-27-fluid-checkpoint.md)。

## 可用原型与保留代码

- 显式 `--local-cells`：world/LBM验证场支持近场软体与远场数量；loop lab及local Stokes组合拒绝。

- 默认世界：Rust 模拟＋Rapier＋Godot，11 节点、12 血管段，72 个十二节点红细胞；解析血流驱动，六次解析采样。
- 显式 `--local-stokes`：固定双重叠区域、二维准静态 Stokes、少量有限阻力标记耦合，默认 f64 faer 联合稀疏 LU。
- 公共流体、可选 GPU 后端和实验入口；GPU 依赖不进入默认游戏路径。正式 LBM 通过默认关闭的 feature 显式启用。
- 旧 GPU Stokes 实验保留 FGMRES／分块 MG、可选分段 profiling、已验证的状态／解回读缓冲复用；尚未进入 Godot 默认路径。
- 云实例 `57949215` 已停止，实际状态 `exited`，当前／目标状态 `stopped`；磁盘保留。见[最后停机回执](research/experiments/2026-10-27-modular-mg/validation/wgpu-readback-reuse/stop-receipt.json)和[云端约定](../AGENTS.md)。密钥只保存在被忽略的 `.env` 中。

- GPU LBM 独立研究程序：Rust＋wgpu 27／WGSL、D2Q9 MRT＋Guo 外力，18 解析工况和 20 次实卡性能轨迹已完成。新实例 57988490 已取回结果并确认 stopped／exited，见[本轮停机回执](research/2026-10-27-ns-lbm/lbm-vast/stop.json)。
- 正式窗口本轮实测：源码 `7727c60`，RTX 3060、驱动 580.159.03；10 个失败案例和 303 行完整帧前缀已保存，18 个原始文件及源码／二进制哈希已核对。结果收回后再次确认实例 `57988490` stopped／exited，见[最新停机状态](research/2026-10-28-lbm-streaming-window/hardware-rtx3060-20261028/final-stop-status.json)。

## 尚未解决

1. 48×48 大窗口包含泵段后缺少域内泵压升，背景流向错误；代数残差通过不代表该驱动正确。
2. 四个有限阻力标记允许透流，严格封堵和全图拥堵反馈没有实现；游戏所需物理效果与误差预算尚未定清。
3. Stokes原型仍是固定双区域；LBM已有移动窗口，普通细胞已有显式数量窗口。全镜头覆盖契约、更一般实体调度、LBM联合交接长测和交接手感尚未完成验收。
4. 旧 GPU Stokes 的命令组织与 MG 小任务成本仍高，保持暂停；LBM 已接入 LSQ-FD＋均值控制；指定场景的机械耦合和5次移窗已实测；尚需处理Godot整体性能、迁移尖峰，并单独安排更广泛场景及物理精度验收。
5. 地图二次 Bézier 末项 `c*t` 与标准 `c*t²` 不同，需独立核对；本轮未动地图。
6. 当前玩家直径 1.7、细胞初始直径 1.4，默认视口约 53.3×33.3 单位；64×48／256×192 局部域与完整固定腔室均只是讨论方案，未确定最终范围。

## 后续范围

当前已完成分叉验证场、游戏性能定位及获确认的静态缓存/剔除/细胞窗口本地实现。Godot整体成本、移窗prepare尖峰及首次editor导入退出异常保留待办，不自动追加长测、优化或改动默认地图。GPU NS 保留补充对照价值，不是采用 LBM 的前置条件。CUDA、分级耦合和房间调度未获实施确认。

心房／心室级开阔区域及模拟域外血流已进入讨论，具体关卡范围与流体架构待定。战斗、治疗、器官关卡和远处演化尚未完成。

## 文档入口与验证

- [本阶段性能交接](research/2026-10-29-performance-checkpoint.md)：归档范围、运行方式、模型取舍、证据与未验收项。

- [可玩验证场与实卡性能](research/2026-10-29-lbm-gameplay/README.md)：当前游戏接入证据、启动方法与性能障碍。

- [LBM 物理文档](physics/lbm-fluid.md)：当前算法与近似，含均值控制的质量/动量代价。
- [本轮接入记录](superpowers/plans/2026-10-29-lbm-integration.md)：正式化状态与实际验证。

- [LBM 阶段决策](research/2026-10-27-ns-lbm/lbm-decision.md)：当前采用决定、证据与待定应用方案。
- [Stokes 阶段记录](research/2026-10-27-fluid-checkpoint.md)：上一阶段模型／近似／算法与结论。
- [研究索引](research/README.md)：逐项历史报告、负例和原始数据。
- [完整流体模块与实验说明](research/experiments/2026-10-27-modular-mg/README.md)。
- [世界小地图设计](superpowers/specs/2026-10-22-world-small-map-design.md)、[世界实施记录](superpowers/plans/2026-10-22-world-small-map.md)、[整体架构](superpowers/specs/2026-10-21-world-architecture-design.md)。

最近资源复用修改已通过相关 fluid／GPU 模块测试、workspace 检查、软件已知解／零 RHS 与时间戳检查，以及物理 GPU 的 384 次回放。192 对前后版本的数值诊断和工作量计数一致。暂停整理只做记录、来源与链接核对，没有新增实验。

2026-10-28 本轮执行正式窗口实卡实验并保存失败证据，随后完成本机独立因果调查；没有修改生产数值代码或运行 UI 交互。原始哈希、复现源码、控制日志与限制均保存在本轮实卡目录，不与旧 Stokes 或周期边界研究结果混称。
