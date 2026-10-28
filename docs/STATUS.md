# 当前状态与待办

更新：2026-10-29。阶段归档分支：`main`。

用户已要求将本会话的研究、算法接入、可玩验证场和实测记录整理后合并至 main。阶段实现与测量已完成，当前待办依次为 Godot 整帧/同步开销定位、移窗 prepare 尖峰、首次 editor 导入退出异常；默认地图与更广泛几何的接入另行安排。继续工作先读下方实测报告和物理文档，不将旧计划中的后续建议视为自动执行授权。

## 阶段状态

**已完成可玩 LBM 分叉验证场和实卡机械耦合/移动回放；完整游戏帧率仍未达标。** `--local-lbm --lbm-validation` 使用真实机器人与 72 个十二节点细胞，固定与移动各完成 120 预热＋3600 测量帧。玩家实际往返并完成 5 次移窗，运行守卫无失败。Rust 完整模拟步 p95 为 12.48/12.15 ms，移窗峰值 24.86 ms；Godot 默认 GL 绘制在该 RTX3060/Xorg 云环境中帧间隔 p95 为 71.40/70.39 ms。无绘制及驱动同步对照已完成，下一步需定位 Godot 绘制/引擎/流体同步共存成本，不能将纯流体预算当作游戏 FPS。见[完整实测](research/2026-10-29-lbm-gameplay/README.md)。

首次 headless editor 导入退出存在已复现的 134 异常，根因未明；cached import 和实际游戏运行/退出为 0。所有结果已收回核验，实例 58133143 已 stopped/exited。

**用户已批准将 LSQ-FD 给定速度边界和帧末平均密度控制接入现有可选 LBM 模块，本轮已在接入分支实现。** 沿用 D2Q9 MRT＋Guo、48×48 移动窗口、图网络边界与现有 Rapier 耦合；`lbm-gpu` 默认关闭，默认游戏仍用解析流。算法、物理代价及适用范围见 [LBM 物理文档](physics/lbm-fluid.md)，接入与检查见 [实施记录](superpowers/plans/2026-10-29-lbm-integration.md)。

固定 `fork_a Fine` 的 RTX 3060 成对 10000 帧实验完成：控制后平均密度残差最大约 2.12e-10，纯流体预算平均 1.90 ms、p95 2.10 ms。它仍持续补偿质量，也改变总流体动量；不能作为自动守恒、完整游戏性能或机械耦合精度证明。见 [预算与长测](research/2026-10-29-lbm-budget-longrun/README.md)。默认 small_world 开口、边界直接切分叉、近切向/欠分辨开口仍不支持；当前没有首发地图验收结论。

此前固定速度重建的 10 个实卡失败案例和 [Sol high 因果调查](research/2026-10-28-lbm-streaming-window/hardware-rtx3060-20261028/independent-investigation.md)保留为原算法负例。本轮沿用精确速度/体积流量契约，不改成固定密度出口；此前周期边界研究也不构成正式开口窗口验收。

RTX 3060 上，Re=100、完整 192²／256²、72 细胞的流体加耦合 p95 为 2.44／3.93 ms，整步为 6.77／8.83 ms，不含渲染。低 Re 高黏度工况的成本限制保留。详见[结果报告](research/2026-10-27-ns-lbm/lbm-results.md)。

上一阶段 GPU Stokes 性能优化仍暂停；其约 46–184 ms／次的回放是另一种算法的历史结果，不代表本次 LBM。旧记录见[Stokes 阶段交接](research/2026-10-27-fluid-checkpoint.md)。

## 可用原型与保留代码

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
3. Stokes 原型仍是固定双区域；LBM 已有移动窗口，但全镜头覆盖、全图调度、远处实体装卸和交接手感尚未完成验收。
4. 旧 GPU Stokes 的命令组织与 MG 小任务成本仍高，保持暂停；LBM 已接入 LSQ-FD＋均值控制；指定场景的机械耦合和5次移窗已实测；尚需处理Godot整体性能、迁移尖峰，并单独安排更广泛场景及物理精度验收。
5. 地图二次 Bézier 末项 `c*t` 与标准 `c*t²` 不同，需独立核对；本轮未动地图。
6. 当前玩家直径 1.7、细胞初始直径 1.4，默认视口约 53.3×33.3 单位；64×48／256×192 局部域与完整固定腔室均只是讨论方案，未确定最终范围。

## 后续范围

当前已完成用户确认的分叉验证场和游戏性能测量。Godot整体成本、移窗prepare尖峰及首次editor导入退出异常保留待办，不自动追加长测、优化或改动默认地图。GPU NS 保留补充对照价值，不是采用 LBM 的前置条件。CUDA、分级耦合和房间调度未获实施确认。

心房／心室级开阔区域及模拟域外血流已进入讨论，具体关卡范围与流体架构待定。战斗、治疗、器官关卡和远处演化尚未完成。

## 文档入口与验证

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
