# 当前状态与待办

更新：2026-10-28。分支：`feat/engine-foundation`。

## 阶段状态

**GPU LBM 已有默认关闭的正式模拟路径，但当前开口窗口未通过数值验收。** `simulation/lbm-gpu` 接入 48×48 移动窗口、图网络边界与 Rapier 耦合，Godot 可显式选择；默认游戏仍使用解析流体。任务 1–6 已实现；正式路径的 RTX 3060 实验已经执行，10 个案例全部在约 0.27–0.72 秒完整帧累计时间后失败，没有可接受的性能 p95 或首发配置。见[实施计划](superpowers/plans/2026-10-28-lbm-streaming-window.md)和[本轮实卡记录](research/2026-10-28-lbm-streaming-window/hardware-rtx3060-20261028/README.md)。默认 small_world 的 LBM 开口几何仍不支持。

全新独立 Sol high 的[因果调查](research/2026-10-28-lbm-streaming-window/hardware-rtx3060-20261028/independent-investigation.md)确认当前低 lattice 黏度下，固定速度出口存在增长的横向边界模。纯流体生产 API 的四个基准逐一复现实卡失败帧；模式注入的实测增长率与独立线性化预测一致。固定密度出口仅为因果控制，会改变精确流量契约；尚无经过验证的生产修复。此前周期边界研究不构成该开口窗口验收。
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
4. 旧 GPU Stokes 的命令组织与 MG 小任务成本仍高，保持暂停；LBM 当前固定速度出口的低黏度数值不稳定已经确认，需先确定符合流量约束的稳定出口方案，再恢复配置／性能验收。
5. 地图二次 Bézier 末项 `c*t` 与标准 `c*t²` 不同，需独立核对；本轮未动地图。
6. 当前玩家直径 1.7、细胞初始直径 1.4，默认视口约 53.3×33.3 单位；64×48／256×192 局部域与完整固定腔室均只是讨论方案，未确定最终范围。

## 后续范围

后续先确定并确认稳定出口方案；当前实卡实验已有失败结论，不选取首发 profile，也不把 Coarse 视为已验证降级档。新的边界契约或数值方案须按项目约定先确认，本次仅完成独立调查，没有改生产求解器或继续租用 GPU。GPU NS 保留补充对照价值，不是采用 LBM 的前置条件。CUDA、分级耦合和房间调度未获实施确认。

心房／心室级开阔区域及模拟域外血流已进入讨论，具体关卡范围与流体架构待定。战斗、治疗、器官关卡和远处演化尚未完成。

## 文档入口与验证

- [LBM 阶段决策](research/2026-10-27-ns-lbm/lbm-decision.md)：当前采用决定、证据与待定应用方案。
- [Stokes 阶段记录](research/2026-10-27-fluid-checkpoint.md)：上一阶段模型／近似／算法与结论。
- [研究索引](research/README.md)：逐项历史报告、负例和原始数据。
- [完整流体模块与实验说明](research/experiments/2026-10-27-modular-mg/README.md)。
- [世界小地图设计](superpowers/specs/2026-10-22-world-small-map-design.md)、[世界实施记录](superpowers/plans/2026-10-22-world-small-map.md)、[整体架构](superpowers/specs/2026-10-21-world-architecture-design.md)。

最近资源复用修改已通过相关 fluid／GPU 模块测试、workspace 检查、软件已知解／零 RHS 与时间戳检查，以及物理 GPU 的 384 次回放。192 对前后版本的数值诊断和工作量计数一致。暂停整理只做记录、来源与链接核对，没有新增实验。

2026-10-28 本轮执行正式窗口实卡实验并保存失败证据，随后完成本机独立因果调查；没有修改生产数值代码或运行 UI 交互。原始哈希、复现源码、控制日志与限制均保存在本轮实卡目录，不与旧 Stokes 或周期边界研究结果混称。
