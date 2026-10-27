# 可插拔流体后端与 GPU 分块多重网格

用户已确认三个 crate、统一运行入口，以及 FGMRES＋分块预条件＋速度块多重网格路线。实现保留原 MAC Stokes 方程、自然牵引开口和细胞耦合；默认局部求解器仍为 CPU LU。

**Windows 实机补充：** [GTX 1050 Ti 的 1／2 次 V-cycle 对照](windows-gtx1050ti.md)已完成，Windows 包装入口验证通过。两组共 96 次 GPU 求解全部通过原方程检查；默认两次配置各规模均值约 130／202／306 ms，仍慢于 CPU LU 的约 2.25／1.76／10.31 ms。下文的“本轮”及软件 GPU 记录保留开发机首次验证口径。

## 模块与接口

| 模块 | 职责 | 依赖方向 |
| --- | --- | --- |
| `blood-fluid` | CSR／Stokes 问题、数据格式、原方程审核、LU、CPU FGMRES-MG | 不依赖 simulation、Godot、wgpu |
| `blood-fluid-gpu` | wgpu 设备、旧 GMRES 基线、新 GPU FGMRES-MG | 依赖 blood-fluid |
| `fluid-bench` | 后端选择、真实问题采集、回放、设备探测、已知解检查 | 依赖 simulation 与 blood-fluid；GPU 为可选 feature |
| `simulation` | 地图、MAC 组装、边界驱动、细胞与 Rapier 耦合 | 只依赖 blood-fluid，通过工厂选择求解器 |

`SolverFactory::prepare(Arc<StokesProblem>)` 返回 `Box<dyn PreparedSolver>`；`solve(rhs, output)` 返回收敛状态、迭代数、网格层级和 GPU 工作量。固定矩阵的分解／层次可跨 RHS 复用。`LocalFluidOptions` 显式携带 n、h、两个区域原点和求解工厂，重置保留这些选项。

当前提供 `cpu-lu`、`cpu-fgmres-mg`、`gpu-gmres`、`gpu-fgmres-mg` 四个选择。CPU 回放可单独构建，默认 Godot 构建不引入 GPU 运行依赖。此接口是 Stokes 线性求解接口；LBM 将是另一个物理模型，本轮未实现。

## 算法与成本

原方程 `K=[A,Cᵀ;C,0]` 保留，先作对称缩放 `K'=DKD`。右分块三角预条件先用负压力质量近似作用于压力，再校正速度 RHS，用多重网格近似求解速度块。FGMRES 保存预条件后的 Z，最终解用 Z 更新。

速度块使用连通聚合、一次平滑延拓、Galerkin `Ac=PᵀAP`、每层三次前／后 Jacobi 平滑和不超过 64 阶的粗层精确逆。u／v 聚合分开；几何相近但没有矩阵连接的分支不能直接合并。可选每步 1–4 次残差校正 V-cycle，默认 2 次。

新 GPU 路径将层次、向量和 Krylov 基驻留设备；使用独立动态 uniform 偏移，避免同批参数被覆盖。两次正交化、Givens、Hessenberg 更新和回代在设备内执行；每 8 次外层迭代读取一次状态，每次重启核对真实残差，最后回读解并审核原方程。预算耗尽和原方程审核失败均返回失败，不自动放宽容差。

**几十次迭代仍可能很贵。** 每次外层迭代含一到两组 V-cycle、SpMV、正交化与归约。减少回读解决同步问题，不能消除这些工作。`dispatches` 是实际安排的 kernel 数，提前收敛后批次尾部可能由设备标志跳过有效计算；必须同时看完整 `mean_solve_ms`、提交与回读。软件适配器耗时不能预测消费级 GPU。

## Windows／云端运行

Windows 双击根目录 `run-gpu-experiment.cmd`，或执行 `./scripts/gpu-experiment.ps1`。需要 Rust、Python 3、系统链接工具和图形驱动；玩家将来运行打包程序，不需安装 Rust／CUDA／ROCm。Windows 包装脚本本轮未在 Windows 实机复测。

Linux 从仓库根目录运行，需要可用 Vulkan 驱动和 loader：

```bash
python3 scripts/fluid-experiment.py --out /tmp/blood-mg-hardware
```

默认采集 small64／wide96／wide192 的两个区域，各 240 帧；运行 CPU LU、CPU FGMRES-MG、新 GPU FGMRES-MG，每域前四个 RHS、两轮反序。默认 restart=32、max-iter=200、rtol=1e-4、mg-cycles=2。旧 GMRES 仅作为显式负对照选项。输出目录必须是新的，`summary.json` 和 `manifest.json` 位于其根目录；后者记录命令、源码／二进制／输入哈希和原始结果。`CARGO_TARGET_DIR` 可复用构建缓存。

```bash
# CPU 独立运行；不构建 wgpu
python3 scripts/fluid-experiment.py --cpu-only --out /tmp/blood-mg-cpu

# 复用数据，比较一次 V-cycle；每个所选场景必须有两个区域
python3 scripts/fluid-experiment.py --data /tmp/blood-mg-hardware/data \
  --mg-cycles 1 --out /tmp/blood-mg-c1

# 已构建后的单项回放
cargo run --release --locked -p fluid-bench --features gpu -- run \
  --backend gpu-fgmres-mg --case /tmp/blood-mg-hardware/data/wide192/region-0 \
  --limit 8 --mg-cycles 2 --max-iter 200 --rtol 1e-4
```

无物理 GPU 时可显式加 `--allow-software` 做功能诊断，记录始终标为 `hardware_measurement:false`。默认拒绝软件适配器。失败保留原始 JSON／日志并退出非零，不能列作有效加速结果。旧版 `prepare.py` 依赖历史源码结构，新代码统一使用 `fluid-bench capture` 或上述脚本；原 Windows 1050 Ti 档案保留在[旧实验](../2026-10-27-gpu-stokes/windows-gtx1050ti.md)。

## 验证口径与后续

每个解都在原始 f64 K 上核对全系统相对残差、相对动量 RMS 和连续性 RMS，三者均需不超过 rtol，且解有限；速度相对 LU 的 L2 差和压力最大绝对差另报。

原始矩阵、RHS 与 LU 解由新采集入口重新运行三个规模各 240 帧，六域共 108 个二进制文件与重构前逐字节一致。实际运行记录见本目录 `validation/`。这里的回放不包括 Rapier、渲染竞争或动态范围调度；48×48 域包含泵段后的驱动缺失仍存在，代数收敛不能证明背景流向正确。

本轮每个规模各 15 个真实 RHS，两区域合计 45 个。CPU LU、CPU FGMRES-MG 的 1／2 次 V-cycle 配置、新 GPU 的 2 次配置全部通过原方程 `1e-4` 检查。GPU 使用 llvmpipe 软件适配器，仅验证算法与设备执行路径。

| 场景 | CPU 每步 1 次 V-cycle 的迭代数 | CPU／软件 GPU 每步 2 次的迭代数 | 软件 GPU 回读次数 |
| --- | --- | --- | --- |
| small64 | 17–18 | 15–16 | 5 |
| wide96 | 36–51 | 32–46 | 7–10 |
| wide192 | 38–57 | 34–49 | 9–11 |

新 GPU 原方程相对残差最大约 `2.60e-5`，速度相对 LU 的 L2 差最大约 `8.04e-4`（0.0804%）。原方程残差和解误差是不同指标，不能据此断言玩家无感。一次 V-cycle 的 CPU 对照虽然全部通过残差门槛，wide96 的速度相对误差最高约 0.604%；减少 V-cycle 时也要查看解差。

small64 每次求解安排 1,564 个 dispatch，wide192 为 4,699–6,571 个。回读已显著减少，kernel 数仍多；本轮不以迭代数或软件耗时宣称加速。两次 V-cycle 默认保持，物理设备上再比较完整成本。

验证命令及结果：

- `bash scripts/check.sh`：格式、workspace 检查、simulation 既有与重置／交接检查、扩展构建、Godot 两场景 headless 启动通过。
- `cargo test --offline --locked -p blood-fluid -p blood-fluid-gpu`：FGMRES、MG 连通性和两套 shader 检查通过；实际 target-dir 和命令见[验证摘要](validation/summary.json)。
- 旧 `lab` 的 `cargo test --offline --manifest-path …/lab/Cargo.toml`：兼容检查通过。
- 新统一脚本的 CPU 独立运行与软件 GPU 全样本回放通过；已知解、零 RHS、真实粗层执行通过。迭代预算仅 2 步时返回 `iteration_limit`；默认硬件探测拒绝软件适配器；缺少所选场景或第二区域时明确报错。
- 只读代码审阅完成，修正实验脚本可能漏跑所选场景的问题。Windows PowerShell 包装及物理 GPU 性能未在本机验证。

`validation/` 保存各配置原始 JSON、源码／输入／二进制哈希、采集等价性和检查日志；完整矩阵不重复入库，可用采集命令重建。原始 JSON 中的软件耗时保留为诊断数据，不能用来计算硬件加速比。

下一步在物理 GPU 上运行同一入口，比较 1／2 次 V-cycle 的总耗时、初始化、提交和回读，并与 CPU LU 比较；再决定是否进入游戏运行路径。尚未实测物理 GPU 性能，也没有将 GPU 设为默认或新增玩家环境门槛。
