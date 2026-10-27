# GTX 1050 Ti：模块化 GPU 多重网格实测

日期：2026-10-27。源码提交 `de0733c`，运行期间未修改源码。硬件为 i7-8750H（6 核 12 线程）、32 GB 内存、GTX 1050 Ti 4 GB，NVIDIA 驱动 582.78，实际 wgpu Vulkan／DiscreteGpu，非软件适配器。Rust/Cargo 1.97.1，Python 3.12.14。

## 运行与原始记录

默认 Windows 入口 `powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/gpu-experiment.ps1` 实际执行成功。三个规模各采集 240 帧、两个区域，共 45 个真实 RHS／LU 解。六域的矩阵 CSR 与全部 RHS／LU 解共 108 个二进制文件，与此前 Windows 负基线逐字节一致，见 [equivalence.json](validation/windows-gtx1050ti-c2/equivalence.json)。

默认配置为每次预条件两次 V-cycle、restart=32、max-iter=200、rtol=1e-4。每域前四个 RHS，正反序两轮；CPU LU 每样本十次回代，CPU／GPU FGMRES-MG 每样本一次。随后复用同一数据，按实验计划补充一次 V-cycle 的 CPU／GPU 匹配对照，其他参数和两轮顺序不变：

```powershell
python scripts/fluid-experiment.py --data target/gpu-experiments/20261027-180835/data --out target/gpu-experiments/20261027-mg-c1-gtx1050ti --mg-cycles 1 --backends cpu-fgmres-mg gpu-fgmres-mg
```

本机实际使用 Codex 自带 Python，设置 `PYTHONUTF8=1`。完整矩阵和构建／运行日志位于 `target/gpu-experiments/20261027-180835/`；一次配置输出位于 `target/gpu-experiments/20261027-mg-c1-gtx1050ti/`。可再生矩阵和编译产物不入库。

两组均 `completed_successfully:true`、`hardware_measurement:true`，所有子进程退出 0。以下归档保留逐次结果、命令、源码／二进制／输入哈希、硬件和汇总：

- [两次 V-cycle：原始结果](validation/windows-gtx1050ti-c2/summary.json)、[运行 manifest](validation/windows-gtx1050ti-c2/manifest.json)、[聚合指标](validation/windows-gtx1050ti-c2/metrics.json)。
- [一次 V-cycle：原始结果](validation/windows-gtx1050ti-c1/summary.json)、[运行 manifest](validation/windows-gtx1050ti-c1/manifest.json)。

## 数值结果

物理 GPU probe 和新版 smoke 通过，包含真实粗层执行、已知解和零 RHS。默认配置 CPU LU、CPU FGMRES-MG、GPU FGMRES-MG 各 48 次样本回放全部通过；一次配置的 CPU／GPU 各 48 次也全部通过。两组共 96 次 GPU 求解均收敛，原始 f64 方程的全残差、相对动量 RMS 与连续性 RMS 均不超过 1e-4。

| 规模 | GPU 两次 V-cycle 迭代数 | GPU 一次 V-cycle 迭代数 | 两次：最大速度相对 L2 差 | 一次：最大速度相对 L2 差 |
| --- | ---: | ---: | ---: | ---: |
| small64 | 15–16 | 17–18 | 0.00326% | 0.00558% |
| wide96 | 32–42 | 36–51 | 0.0805% | 0.604% |
| wide192 | 34–46 | 38–57 | 0.0780% | 0.230% |

两次配置的最大原方程相对残差约 2.60e-5，一次配置约 3.30e-5。残差通过与速度解差是不同指标；不据此宣称玩家无感，也不自动改默认 V-cycle 数。

## 求解耗时

下表为两个区域、两轮、每域四个 RHS 共 16 个记录的算术平均；CPU LU 每条记录先平均十次回代。计时包含求解入口内的工作，初始化另计，不含 Rapier、Godot 绘制和域调度。

| 规模 | CPU LU ms | CPU FGMRES-MG（两次）ms | GPU FGMRES-MG（两次）ms | GPU FGMRES-MG（一次）ms |
| --- | ---: | ---: | ---: | ---: |
| small64 | 2.25 | 95.0 | 130.4 | 85.1 |
| wide96 | 1.76 | 267.2 | 201.7 | 215.8 |
| wide192 | 10.31 | 1117.5 | 305.6 | 222.9 |

两次 GPU 配置的逐样本耗时范围为 small64 63.6–345.5 ms、wide96 113.2–273.6 ms、wide192 206.6–400.4 ms。一次 wide96 有 741.7 ms 长尾。两次配置先于一次配置运行，未将两种 V-cycle 配置交错；桌面应用、功耗与温度未控制，不能将均值差都归因于参数。

GPU 初始化均值约 small64 1.18 秒、wide96 0.90 秒、wide192 1.82 秒，包含设备／pipeline、CPU 层次构建及上传；固定问题的这些成本在多次 RHS 间复用。

对照旧版同数据集：旧 GMRES 48 次均在 400 次后失败，GPU 约 1.4–1.7 秒；新版收敛并缩短耗时。但旧版未达到同等精度，不能将两者时间比当作同精度加速比。

## 工作量与结论

| 规模 | 两次：GPU dispatch 数 | 两次：回读数 | 两次：提交数 | 一次：GPU dispatch 数 |
| --- | ---: | ---: | ---: | ---: |
| small64 | 1564 | 5 | 10 | 1356 |
| wide96 | 3116–4675 | 7–10 | 14–21 | 2259–3155 |
| wide192 | 4699–5635 | 9–10 | 19–21 | 2659–4243 |

**收敛问题已在本次样本中解决，实时性能问题仍未解决。** GPU 对较大的问题比当前 CPU FGMRES-MG 候选更快，但 CPU 版本为 f64、GPU 为 f32，不是隔离硬件的比较。CPU LU 对当前固定矩阵仍明显更优，继续作为默认局部后端。

回读从旧版每次 415 次降至个位数或十次左右，然而仍有上千个小 kernel。现有数据支持继续分析 kernel 数、命令编码、每次 pass 和 GPU 执行成本；本轮没有 GPU timestamp 或分段 profiling，不能断言哪一项是唯一瓶颈，也没有自动修改 shader 或合并命令。

本轮只执行实验与结果归档，Windows 包装入口得到实机验证。未修改求解算法、Godot 默认、模拟参数或玩家环境门槛。已知大窗口内部泵驱动缺失仍存在，代数收敛不构成流向／游戏体验验收。
