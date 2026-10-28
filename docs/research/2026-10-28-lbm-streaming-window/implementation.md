# LBM 移动窗口实施记录

已确认的正式边界采用 `blood-fluid-gpu::lbm`；`simulation` 通过默认关闭的 `lbm-gpu` feature 依赖它，Godot 桥接负责显式选择及错误展示。48×48 窗口按 16×16 区块迁移，Fine 和 Coarse 分别为 192² 和 96²。图网络提供背景及开口流量，Rapier 保有实体状态；局部 LBM 不向图网络反馈压力或流量。任务 1–6 的实现与对应检查见[实施计划](../../superpowers/plans/2026-10-28-lbm-streaming-window.md)，第 7 项的[原始数据与运行说明](README.md)保留软件功能实验和硬件测量要求。

Terra medium 实施、Sol high 按任务审阅后，Sol high 完成了 `7d3d334..7727c60` 的[整轮代码审阅](review.md)：任务 1–6 和任务 7 本地准备通过，无 Critical 或 Important 问题。两个非阻塞 Minor 已记录；实卡实验及配置结论仍待授权和验证。

后续用户授权的[RTX 3060 实验](hardware-rtx3060-20261028/README.md)已完成，10 个案例全部失败，没有可用首发配置。独立 Sol high 通过纯流体复现、边界单变量控制和定量模式注入，确认固定速度出口在当前低 lattice 黏度下的增长模；见[独立报告](hardware-rtx3060-20261028/independent-investigation.md)。这项实测结论取代此前尚待验证的状态，代码审阅通过不等于数值／游戏验收通过。生产数值代码未改，实例已停止。

默认压力 12 的叉口 Fine 软件试验出现失稳，固定窗口时也可复现；其数值原因未确定，该工况暂不支持。此结果不能推断所有 Fine 工况，也不能替代实卡测量。默认 small_world 的 LBM 开口几何目前不支持。Coarse 的短时软件功能结果并非性能或长期稳定性验收；配置、缓冲及迁移瞬态尚无首发结论。

窗口在每次已有宏观场回读后验证流体密度为正、密度及速度有限，并将错误传播到 `try_tick`；Godot 桥接已有错误暂停处理。无效结果可能发生在已部分推进的帧内，不自动恢复。回归 `cargo test --offline --locked -p simulation --features lbm-gpu invalid_gpu_macrostate_returns_error_from_batch -- --ignored --nocapture` 使用真实 WGSL 有限输入使宏观状态无效，验证推进返回错误。此修改不改变数值方程，也不修复上述 Fine 失稳。软件测试不构成硬件性能证据。
