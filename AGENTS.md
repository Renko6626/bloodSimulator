# 项目工作约定

## 当前流体路线（2026-10-29）

用户已确认 Rust＋wgpu／WGSL、D2Q9 MRT＋Guo 外力路线，并阶段性批准 LSQ-FD 给定速度边界与帧末平均密度控制，确认按推荐方案接入现有可选 `simulation/lbm-gpu` 路径。本轮已在 `feat/lbm-lsq-integration` 实现：`blood-fluid-gpu` 复用已有 faer 0.24.4 运行时生成权重，simulation 显式帧末修正并刷新缓存；不扩大 Godot/snapshot 接口，默认游戏仍用解析流。状态与检查见 [接入计划](docs/superpowers/plans/2026-10-29-lbm-integration.md)，算法见 [物理文档](docs/physics/lbm-fluid.md)。

RTX 3060 上固定 `fork_a Fine` 的 10000 帧均值控制实验已完成，纯流体 p95 约 2.10 ms；这是固定几何、无机械的证据，不是完整游戏验收。均值控制持续补偿质量并改变总流体动量，不代表边界自动守恒。旧固定速度重建的实卡失败与因果调查保留为历史负例，见 [原记录](docs/research/2026-10-28-lbm-streaming-window/hardware-rtx3060-20261028/README.md)。固定密度出口仍只是旧因果控制，不替换给定速度/流量契约。

默认 `small_world`、开口切分叉、角点、弯曲/歧义和近切向开口限制未放宽；不得据选定斜管案例声称任意斜率都可用。选定验证场的机械耦合/短程移窗证据见下节；更广泛耦合、移窗长程、绝对压力响应和物理精度尚未验收。GPU NS 对照不再是采用 LBM 的前置条件，下方 Stokes 暂停约定仍有效。最近实验实例 `58133143` 已取回结果并确认 stopped／exited；本轮模块准备不自动追加云实验。

## 可玩验证场与性能（2026-10-29）

用户已确认并完成 `feat/lbm-gameplay-validation`：`--local-lbm --lbm-validation` 进入独立分叉验证场，复用既有机械耦合，72个十二节点红细胞局部初始化，正常WASD/暂停/重置及显式输入回放。固定/移动各60秒测量完成，实际5次移窗，运行守卫通过；不是任意路径或物理精度全面验收。源码与原始数据见[报告](docs/research/2026-10-29-lbm-gameplay/README.md)。

本轮RTX3060/Xorg中，Rust完整模拟p95约12ms，而Godot默认GL绘制回放的帧间隔p95约70–71ms，完整游戏尚未达60FPS。无绘制和进程级vblank对照已记录；不得把约2.10ms的旧纯流体预算当作游戏FPS，也不能把同步等待全部解释成GPU算子耗时。移窗prepare有尖峰。首次headless editor import/quit有134异常，cached import及真实gameplay退出0，根因未明。实例58133143已取回数据并确认stopped/exited，后续优化/云实验需新的任务安排。

## 上一阶段 Stokes 研究暂停

2026-10-27，用户要求阶段性暂停 Stokes／GPU 性能研究。当前统一交接文档是 [流体实验阶段记录](docs/research/2026-10-27-fluid-checkpoint.md)。

保留已验证代码与原始记录；旧报告中的下一步不构成自动继续优化、启动云实例或试新后端的安排。用户重新明确安排相关工作时再继续。

## Vast.ai 实验收尾

用户已授权：本项目的云端实验完成后，主动使用项目 `.env` 中的 `VAST_API_KEY` 停止本次使用的 Vast.ai 实例，无需重复确认。

- 先取回并核对实验结果、日志和必要的来源记录，再停止实例；有后续正在执行的实验步骤时不要提前停止。
- 用本次实例的公网地址、SSH 端口映射核对实例 ID，只操作本次实验实例。
- 调用平台的 stop 操作，核对目标状态为 `stopped` 且实际容器已停止；本轮平台返回 `cur_state: stopped`、`actual_status: exited`。仅提交请求或目标状态改变不代表已经停止。
- stop 保留磁盘和数据，仍可能产生存储费用；用户本次授权不包含销毁实例或删除数据。
- 密钥仅在本机读取使用，不输出到终端、对话、日志或仓库；`.env` 保持忽略。
