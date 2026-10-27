# 跨厂商 GPU Stokes 实验与云端接手

用户已确认独立 **Rust＋wgpu／WGSL** 实验。这里准备可运行代码和复现步骤，正式模拟、根依赖、Godot 渲染器均保持。

主路线不依赖 CUDA、ROCm、cuDSS。开发服务器需要 Rust 和相应图形运行支持；将来玩家运行的是打包后的程序，只需满足图形驱动／系统运行时要求，不需要科研 SDK 或 Rust 工具链。wgpu 使用 Vulkan／D3D12／Metal，**不等于所有旧显卡都支持**；Windows／macOS 和消费级显卡尚待实测。当前程序不需要窗口或显示器。

## 当前已经有的代码

- `prepare.py` 在冻结生产副本中采集真实系统，默认三个规模、两个区域：16×16／64²，48×48／96²，48×48／192²；保留 72 个细胞、真实驾驶控制和每步 RHS。
- `capture.rs` 导出实际组装 K 的完整 CSR 和 Matrix Market、域几何、代表性 RHS 与 CPU LU 解；所有导出发生在临时副本。
- `lab/` 是独立 Cargo workspace，有自己的锁文件。CPU 参照直接编译当前生产 `StokesFactor`，不另抄一套离散方程。
- GPU 实现 CSR SpMV、并行归约、两次正交化及向量更新；大矩阵、解和 Krylov 基向量驻留 GPU。CPU 处理小 Hessenberg 系统和停止控制，每轮 Arnoldi 回读少量系数，最后回读解。
- 算法为重启 GMRES，`K'=D K D, b'=D b, x=D y`；速度用 `diag(A)`、压力用 `diag(B diag(A)⁻¹ Bᵀ)` 做对称缩放。**没有压力罚项、钉住新自由度，也没有把完整不定系统交给普通 CG。**
- `cloud.py` 检查数据来源，运行硬件探测、小型检查、CPU 参照和正反序 GPU 回放；未收敛、原方程残差超界或无硬件会保留记录并返回失败。

它是 GPU 运算／迭代／同步成本的起点。简单缩放不保证实际分叉系统能在默认迭代预算内收敛，更不保证比 CPU LU 快；这是后续是否需要多重网格的测量依据。

**2026-10-27 Windows 实测：** [i7-8750H／GTX 1050 Ti 基线与原始结果](windows-gtx1050ti.md)已完成。Vulkan 物理 GPU、单元与小型数值检查通过；六域 CPU 参照通过，48 次真实 GPU 回放均达到 400 次迭代上限，未通过残差门槛。下文“本机完成的验证”保留此前 Linux 无物理 GPU 环境的历史记录。

## 云服务器上运行

### Windows 一键实验

在仓库根目录双击 `run-gpu-experiment.cmd`，或在 PowerShell 执行 `./scripts/gpu-experiment.ps1`。需要 Rust 及可用的 Windows C++ 链接工具；首次运行联网下载锁定依赖。脚本优先使用 Codex 自带 Python，也可通过 `-Python C:\path\python.exe` 指定 Python 3。

入口依次执行关键测试、release 构建、物理 GPU 探测、小型数值检查、全部三种规模的真实矩阵采集、CPU 参照和两轮 GPU 回放。使用原定迭代预算与容差；未收敛保留为失败结果。每次写入新的 `target/gpu-experiments/日期时间/`，包含硬件、日志、矩阵和结果；`results/summary.json` 给出最终状态。首次编译和采集可能较慢，此入口不是预编译二进制分发包。

从仓库根目录执行。Rust 最低 1.94；Linux GPU 镜像需提供可用 Vulkan 驱动／ICD 和 loader，**只有 CUDA Toolkit 不保证 Vulkan 可用**。不需要 Vulkan SDK；驱动和系统运行时仍需正确安装。可用 `WGPU_BACKEND=vulkan` 明确选择后端。

```bash
cargo fetch --locked --manifest-path docs/research/experiments/2026-10-27-gpu-stokes/lab/Cargo.toml

python3 docs/research/experiments/2026-10-27-gpu-stokes/prepare.py \
  --out /tmp/blood-gpu-data

python3 docs/research/experiments/2026-10-27-gpu-stokes/cloud.py \
  --data /tmp/blood-gpu-data --out /tmp/blood-gpu-results
```

输出目录必须是新的；重复实验使用新名称。`prepare.py` 默认采集 240 帧，各域保留调用 0、1、3、7、15、31、63、127 中实际发生的样本。调用 0 为初始背景场，其余包含真实耦合 RHS；域的激活时间不同，样本数可以不同。本机这次共得到六域、45 个样本。

`cloud.py` 默认每域前四个样本、CPU 各十次回代、GPU 两轮反序回放，GMRES restart=32、最多 400 轮 Arnoldi、rtol=1e-4；CLI 可显式改变这些值，实际命令全部记录。失败不应自动触发放宽容差或扩大参数搜索。硬件探测失败仍可完成 CPU 参照，但整个硬件实验退出非零；`--cpu-only` 用于明确只验证 CPU。

单项定位命令：

```bash
cargo build --release --locked \
  --manifest-path docs/research/experiments/2026-10-27-gpu-stokes/lab/Cargo.toml \
  --target-dir /tmp/blood-gpu-lab-target

/tmp/blood-gpu-lab-target/release/blood-gpu-stokes-lab probe
/tmp/blood-gpu-lab-target/release/blood-gpu-stokes-lab smoke
/tmp/blood-gpu-lab-target/release/blood-gpu-stokes-lab gpu \
  --case /tmp/blood-gpu-data/wide192/region-0 \
  --limit 4 --restart 32 --max-iter 400 --rtol 1e-4 \
  --out /tmp/blood-wide192-gpu.json
```

`probe`／正式回放默认拒绝 CPU 软件适配器。仅做功能诊断时可显式加 `--allow-software`，包括受限步数的真实 CSR 检查；这类输出标记 `hardware_measurement:false`，不得列为硬件性能。`smoke` 检查 3×3 不定系统、零 RHS 和跨三个工作组归约块的 514 阶已知解；它始终属于功能检查。

## 数据和结果如何解释

每域有 `matrix.json`、`matrix.mtx`、`row_ptr.u32`、`columns.u32`、`values.f64`、`domain.json`、`rhs-NNN.f64`、`solution-NNN.f64`、`samples.csv`。二进制 little-endian，完整向量顺序是速度、压力；`domain.json` 保留 n、h、原点、压力格索引和 u／v 面坐标，供后续多重网格使用。

CPU 参照是 f64 稀疏 LU；GPU 是 f32 迭代，因此这是**完整候选方案对照**，不能把收益全部归为硬件。GPU SpMV 先与同一原矩阵的独立 f64 运算比较；最终解转换回原变量，在原始 K 上计算：

- 全系统相对残差 `||Kx-b||₂ / max(||b||₂,1e-30)`；
- 动量 RMS、相对于 `max(rhs_velocity_rms,1)` 的动量 RMS；
- 连续性块 RMS；
- 速度相对 CPU 解的 L2 差、压力最大绝对差。

`accepted` 要求缩放系统报告收敛，且原系统全残差、相对动量 RMS、连续性 RMS 都不超过 rtol；解差单独报告。它只表示此线性系统的上述数值门槛，不代表玩家无感或完整游戏验收。

`end_to_end_solve_ms` 包含 RHS 上传、所有迭代提交／同步和最终解回读；矩阵上传和设备／pipeline 初始化另记 `setup_ms`。CPU 同样固定因子，单独记录分解和逐次求解。`readback_wait_ms` **包含等待先前 GPU 工作完成**，与总耗时有重叠，不能当纯传输耗时相加或相减。GPU 时间戳、f64 shader、子组指令都不是运行必需特性。

当前每个 primitive 单独提交，参数写入后立即提交；此处不要直接合并 submit，否则共用 uniform 会被后续参数覆盖。后续若合并需使用不同参数存储方式。首版会有明显主机提交／标量同步开销，应先用结果决定是否值得优化。

本实验只回放线性系统，**没有 Rapier 耦合回路、渲染竞争或动态域调度的 GPU 计时**。不能把回放耗时当游戏整步耗时。已有大窗口泵驱动缺失仍保持；线性解正确也不能证明该背景驱动正确。

## 本机完成的验证

实际执行并通过：

```bash
cargo test --offline --locked \
  --manifest-path docs/research/experiments/2026-10-27-gpu-stokes/lab/Cargo.toml \
  --target-dir /tmp/sunyunbo/blood-gpu-lab-target -j 4

python3 docs/research/experiments/2026-10-27-gpu-stokes/cloud.py \
  --data /tmp/sunyunbo/blood-gpu-data-v1 \
  --out /tmp/sunyunbo/blood-gpu-cpu-validation-v1 \
  --target-dir /tmp/sunyunbo/blood-gpu-lab-target --offline --cpu-only --limit 8
```

三项关键检查覆盖 GMRES 不定／零 RHS、缩放与原约束块审计、无设备 WGSL 解析和验证。六域 45 个捕获样本的 CPU 回放通过，原系统相对残差最大约 `3.36e-14`。显式 llvmpipe 功能检查中，3×3／514 阶已知解最大误差约 `2.39e-7 / 1.20e-7`，零 RHS 无迭代。

额外将真实 small64 的预算限制到两次迭代，程序正确返回 `iteration_limit`、`accepted:false` 和退出码 2；没有把有限值或很短的耗时当成收敛。默认 `probe` 对本机 llvmpipe 返回失败，符合硬件门槛。

本机没有可用物理 GPU 测量；没有测试 GMRES 对真实数据的充分预算收敛、物理 GPU 性能、Windows／Metal、长期细胞行为或 Godot 接入。一次范围限定的只读审阅未发现阻断问题。记录口径：`hardware_tests_run` 表示 probe／smoke 成功后进入硬件回放流程，失败的探测尝试仍逐条保存在 `runs`。

小型原始记录见 `validation/`；37 MiB 的可再生矩阵、编译产物和临时工作区保留在 `/tmp`，不纳入源码。源码、锁文件、shader 和采集来源均有哈希记录。

## 后续安排

1. **云端先做真实矩阵回放。** 保留失败原始结果，分别判断是迭代数、提交／归约同步、内存访问还是精度限制；不能只挑 SpMV 内核时间报告加速。
2. **若同步主导，先缩减同步。** 考察设备内标量控制、批量提交；仍保留当前每机械子步需要正确结果的耦合时序，不默认延迟几帧回读。
3. **若迭代数主导，再做 GPU 多重网格。** 先验证速度 SPD 子块、压力 Schur 近似和粗层连接，作为外层 GMRES 的块预条件；分叉、薄血管、开口和内部泵各自要有明确边界处理。不能直接对完整 K 套正定 Poisson 多重网格。
4. **GPU LBM 单列实验。** 从既有 CPU LBM／细胞耦合研究出发，明确碰撞模型、低 Mach、黏度尺度、壁面、泵和内部子步；比较同物理时长下的全部传播、耦合与回读成本。一次 LBM 内步不等于一个游戏步，也不自动解决 Stokes 模型问题。
5. **候选验证后再接游戏。** 消费级 NVIDIA／AMD／Intel 设备验证、CPU 路径保留、Godot 数据共享、渲染争用和域切换逐项决定，不用 A100 结果替代玩家设备判断。

完整实施与后续边界见[计划](../../../superpowers/plans/2026-10-27-gpu-stokes.md)。多重网格和 LBM 当前只列安排，未假装已有实现。

## 将当前工作区带到云端

云端请拉取包含本实验的提交，并检出 `feat/engine-foundation` 分支；旧的 `077d9d8` 不包含本实验及其依赖的 CPU 原型。无需复制本机临时数据，按上面的命令重新采集即可。也可从仓库根目录制作源码包：

```bash
tar --exclude=target --exclude=__pycache__ -czf /tmp/blood-gpu-source.tgz \
  Cargo.toml Cargo.lock crates \
  docs/research/experiments/2026-10-27-gpu-stokes \
  docs/superpowers/plans/2026-10-27-gpu-stokes.md
```

云端解压到新目录后按上面的命令运行，采集器会冻结随包带来的实际生产来源。新服务器不依赖本机 `/tmp/sunyunbo`、旧实验临时目录或 NVIDIA 专用库。

参考：[wgpu 后端](https://wgpu.rs/)、[固定版本 API](https://docs.rs/wgpu/27.0.1/wgpu/)、[Godot Compute Shader 的运行约束](https://docs.godotengine.org/en/stable/tutorials/shaders/compute_shaders.html)。
