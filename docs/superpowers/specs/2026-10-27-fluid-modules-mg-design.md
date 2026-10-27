# 可插拔流体求解与 GPU 分块多重网格

用户已确认：三个独立 crate、统一独立运行入口，FGMRES＋速度／压力分块预条件，速度块使用多重网格。保持 Rust／wgpu／WGSL，不要求玩家安装 CUDA／ROCm。这里记录本轮具体边界，实施不再重复询问已确认路线。

## 目标与边界

- `blood-fluid`：规范化 Stokes 系统、数据／原方程审核、可插拔求解接口、CPU LU 和 CPU 多重网格参照；不依赖 Rapier、Godot 或 GPU。
- `blood-fluid-gpu`：wgpu 设备和数值 kernel，旧 GMRES 基线与新 FGMRES／分块 MG，均实现相同求解接口。
- `fluid-bench`：统一 CLI、实际 simulation 数据采集和跨后端回放；按 feature 选择 GPU，CPU 可独立运行。
- simulation 保留血管图、MAC 离散及细胞耦合职责，通过 `SolverFactory` 构建求解器；独立问题对象携带 CSR、速度／压力切分和网格元数据。默认 CPU LU，GPU 不成为默认游戏依赖。
- 当前四个后端选择都求相同 Stokes 方程。将来 LBM 是独立物理模型，应按几何／驱动／流场进行比较，不强制伪装成 Stokes 线性求解后端。本轮不实施 LBM。

另一可行路线是只整理旧实验 runner，改动较小但保留 simulation 与求解代码的路径耦合；用户选择了上述正式模块抽取。

## 公共接口

`SolverFactory::prepare(Arc<StokesProblem>) -> Result<Box<dyn PreparedSolver>, String>`。

`PreparedSolver::solve(&mut self, rhs, output) -> Result<SolveReport, String>`，不收敛以 report 明确返回，基准保留负例；实时路径不使用未收敛场继续步进。

问题包含 CSR、`velocity_count`、黏度和可选结构化几何。几何记录 n／h／原点／开口／压力格索引／速度面坐标。固定问题跨步复用，动态细胞只改变 RHS。模块化采用 Rust 类型接口和 CLI 选择，不增加动态库加载、注册服务或自动恢复机制。

## 新算法

对原系统 K=[A,Cᵀ;C,0] 使用右预条件 FGMRES，保存 V 与 Z=P⁻¹V。三角分块预条件先近似解负压力 Schur 块，再校正速度 RHS 并近似解 A。

- 速度 A：连通性约束的聚合、平滑延拓、Galerkin `Ac=PᵀAP`，Jacobi 前后平滑，固定小粗层精确解构成 V-cycle；u／v 分量不混合，分离血管不能在聚合中凭空连通。
- 压力：以 Stokes 压力质量阵的 Schur 近似作为首个具体候选，与精确速度块的 CPU 对照验证符号／尺度。它与先前仅对压力使用 `B diag(A)⁻¹ Bᵀ` 的 SIMPLE 负例不同。
- 保留对称变量缩放，解映射回原变量后检查动量、连续性、相对残差及 LU 解差。不放宽 rtol 来掩盖失败。
- GPU 预先上传层次、算子和参数；将 V-cycle 和迭代 kernel 合并到少量提交，避免复用可变 uniform 造成参数覆盖。迭代控制／小 Hessenberg 尽量设备内完成，按批回读状态。

临时精确 A／压力质量近似对照：small64、wide192 的初始 RHS 分别在 15／34 步通过原方程检查；这只是验证分块方向，不能代替 MG 或 GPU 测量。

## 验证与兼容

先保持 CPU LU 轨迹及旧严格流体检查，再验证层次传递、分离连通性和 CPU FGMRES 收敛。GPU 在无硬件环境做 shader、已知解及显式软件功能验证，物理硬件性能留给 Windows／云端。

原始 Windows 1050 Ti 结果与旧历史来源快照不改。旧入口保留明确兼容说明，新的 Windows／云端入口选择新模块，记录具体后端。大窗口域内泵驱动问题保持单独记录，本轮不改地图形状或驱动模型。
