# Windows 消费级硬件 GPU Stokes 基线

日期：2026-10-27。生产来源提交：`8f1a14a2067a1c154910aa8f692bcb3277219837`。本次附 Windows 启动入口及采集器 `.exe` 路径修正；实际源码、数据与二进制哈希见归档 manifest。

## 环境与复现

- Intel Core i7-8750H，6 核 12 线程，32 GB 内存。
- NVIDIA GeForce GTX 1050 Ti，4 GB 显存，驱动 582.78；实际 wgpu 后端 Vulkan、DiscreteGpu，非软件适配器。
- Windows，Rust/Cargo 1.97.1，Python 3.12.14，锁定 wgpu 27.0.1。
- 执行 `powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/gpu-experiment.ps1`。采集 240 帧，三种规模各两个区域，共 45 个真实 RHS／参照解；每域回放前四个 RHS，CPU 每样本十次回代，GPU 正反序两轮。GMRES restart=32、max_iterations=400、rtol=1e-4，未放宽参数。
- 完整数据和构建日志位于本地 `target/gpu-experiments/20261027-155954/`。关键 JSON 已存入 [validation/windows-gtx1050ti-20261027](validation/windows-gtx1050ti-20261027/)，包括硬件、采集来源清单、逐次 CPU／GPU 输出、运行 manifest 和最终 summary。完整矩阵为可再生输出，保留在 target 下。

## 实测结果

三项单元检查、物理 GPU probe 和 smoke 均通过。3×3 已知解最大误差约 2.38e-7；514 阶跨工作组案例误差为零；零 RHS 无迭代。真实矩阵 SpMV 对照的最大相对误差约 2.60e-8。

CPU 的 24 个样本均通过原方程审核，最大相对残差约 3.22e-14。GPU 的 48 次求解全部为 `iteration_limit`、`accepted:false`，均执行 400 次迭代。最终 `completed_successfully:false` 和非零退出表示数值未收敛；完整回放已执行完毕，没有跳过区域或运行崩溃。

| 区域 | 方程阶数 | CPU 回代均值 ms | GPU 端到端均值 ms | GPU 原方程相对残差范围 |
| --- | ---: | ---: | ---: | ---: |
| small64-region-0 | 7400 | 2.80 | 1458.2 | 0.08196–0.1623 |
| small64-region-1 | 5689 | 1.96 | 1393.5 | 0.07039–0.1548 |
| wide96-region-0 | 8191 | 2.58 | 1454.2 | 0.03054–0.04639 |
| wide96-region-1 | 7309 | 1.80 | 1681.8 | 0.04011–0.09588 |
| wide192-region-0 | 33192 | 12.22 | 1610.1 | 0.04893–0.1802 |
| wide192-region-1 | 29565 | 11.08 | 1620.5 | 0.06784–0.2044 |

CPU 为各样本十次回代均值再取平均；GPU 为两轮各四个样本取平均。两者均固定矩阵，初始化、矩阵上传和 CPU 分解另记原始 JSON。GPU 未收敛，这些耗时不能作为同等精度下的加速比。

每次 GPU 求解有 4069 次 dispatch、415 次回读。`readback_wait_ms` 占端到端时间的平均比例约 49.5%，包含等待先前 GPU 工作完成，与总耗时重叠；不能解释为纯传输成本。真实系统残差远高于 1e-4，仅优化同步不足以使当前方案可用。

## 判断与边界

这台旧消费级显卡可以运行现有 WGSL 运算；简单对角缩放的重启 GMRES 对本次真实 Stokes 系统不满足收敛与实时预算。结果不能证明 GPU Stokes 路线整体无效，也不能归因于显卡算力不足。下一步应保留本基线，验证 Stokes 分块预条件、速度子块与压力 Schur 近似，并减少逐轮主机同步。当前不接入正式游戏，不自动扩大迭代预算或放宽容差。

本次只回放线性系统，不含 Rapier 耦合、Godot 绘制和域调度。大窗口内部泵驱动缺失仍是已知模型问题。本机同时运行桌面应用，未控制功耗模式与温度；计时属于本机基线，不代表严格隔离的硬件极限。
