# GPU Stokes 实验准备与云端接手计划

> 执行方式：当前会话由主 agent 完成已授权的独立实验代码；以已批准 spike 的范围执行。用户确认独立 Rust＋wgpu／WGSL，不要求再确认该路线，不进入正式后端接入。

**目标：** 云端获得代码后即可采集真实 Stokes 系统、运行 CPU 参照和 GPU 迭代实验，记录数值正确性及包含同步的实际成本。

**架构：** 独立实验 workspace，生产程序集和根依赖不改。采集器仅在生产源码的临时副本加入矩阵／真实 RHS／LU 解导出；GPU 以可移植 f32 WGSL 执行 CSR 运算和 Arnoldi 向量操作，CPU 只处理小 Hessenberg 系统及停止控制。

**技术：** Rust，固定版本 wgpu 27.0.1、WGSL、既有 faer 0.24.4 CPU LU；Python 标准库编排。首个算法为对称块对角缩放后的重启 GMRES，完整 K 不使用普通 CG。GMRES 是算法／同步基线，不保证无需多重网格即可达到目标。

## 约束

- 用户已确认独立 wgpu 实验，跨厂商为主，不要求玩家安装 CUDA／ROCm／科研 SDK；开发机和玩家运行环境分开。
- Vulkan／D3D12／Metal 运行支持及图形驱动仍是 GPU 路线的实际要求；当前不改变 Godot Compatibility 渲染器。
- 只新增实验目录、计划和导航；继续使用当前包含原型的工作区，保留所有既有未提交更改。
- 泵进入域内后的驱动缺失保持显式记录。线性方程残差通过不代表生理／游戏驱动验收。
- 无硬件 GPU 时不伪造性能；软件适配器只可显式用于小型功能检查，硬件测量拒绝自动降级。
- 不新增恢复队列、通用调度器或正式 GPU 后端接口；不在本阶段迁移 Rapier、多重网格或 LBM。

## 文件与接口

实验根：`docs/research/experiments/2026-10-27-gpu-stokes/`。

- `prepare.py`：冻结当前生产来源，注入临时采集代码，生成 small64／wide96／wide192 两域数据及来源清单。
- `capture.rs`、`driver.rs`：临时副本使用；导出实际 K、CSR／Matrix Market、域几何、代表性真实 RHS、CPU LU 解。
- `lab/`：独立 Cargo workspace 和锁文件。`case.rs` 读取数据、缩放与原方程检查；`gmres.rs` 控制迭代；`gpu.rs` 管理 wgpu；`kernels.wgsl` 实现 SpMV、两次正交化、归约与向量更新；`main.rs` 提供 probe／CPU／GPU 命令与 JSON 记录。
- `cloud.py`：逐数据域启动实验、记录原始结果和失败；不将未收敛或无设备转为成功。
- `README.md`：本机已验证项、云端运行命令、数据解释和后续算法安排。

数据 v1：每域 `matrix.json`、`matrix.mtx`、`row_ptr.u32`、`columns.u32`、`values.f64`、`domain.json`、`rhs-NNN.f64`、`solution-NNN.f64`；二进制为 little-endian，完整 RHS／解按速度、压力顺序。CPU LU 直接调用当前生产 `StokesFactor`，不另写一个离散算子作参照。

## 本阶段任务

- [x] 采集三个规模、两个域的真实系统，核对导出系统的 CPU 解、原块残差和源哈希。
- [x] 完成独立 wgpu 程序、shader、重启 GMRES；矩阵和 Krylov 大向量驻留设备，标记每次标量同步。
- [x] 关键新增性质用简短检查覆盖：不定矩阵的 GMRES、零 RHS、原方程残差检查、无 GPU 的 shader 验证；不扩大正式测试套件。
- [x] 本机完成编译、CPU 回放。物理 GPU 探测失败；显式 llvmpipe 仅做功能检查。格式、脚本及导航核对在交付前完成。
- [x] 写明 cloud probe→CPU→GPU 的命令、失败口径、可转移源码包及未测项；GPU 多重网格／LBM 保留为后续独立实验。

## 云端顺序与决策

1. 确保当前工作区代码已提交后再拉取，或传输 README 指定源码包；不能只拉旧 HEAD 后假定未提交原型已经存在。
2. 确认物理适配器、驱动、后端。先小型 manufactured 不定系统，再 small64→wide96→wide192；保留 CPU f64 LU 参照。
3. 同时报告整体求解延迟、提交／同步与回读、迭代数、原始动量／连续性残差和解差；先检查通过再解释性能。GPU 时间戳不是必需特性。
4. 若 GMRES 归约／同步主导，先考虑设备内标量控制或更少同步的 Krylov；若迭代数主导，再提出 Stokes 分块预条件／多重网格设计。粗层要保留细血管连接和开口，不把完整 K 当正定 Poisson。
5. GPU 多重网格作为后续独立实验：先速度 SPD 子块及压力 Schur 近似，验证多层传递后接外层 GMRES；保持原方程及界面力一致。
6. GPU LBM 作为不同模型候选：单列物理参数、Mach／黏性尺度、内部时间步、边界、泵驱动、细胞反冲量和端到端成本。不能用单次碰撞传播耗时与一次 Stokes 游戏步直接比较。
7. 稳定候选再讨论 Godot 接入、渲染共享和消费级显卡验证。云端 A100 的收益不能代替普通玩家设备结论。

## 验证记录

`prepare.py`：三种规模、六域、45 个真实 RHS／CPU 解采集完成；`cloud.py --cpu-only --limit 8` 完成全部六域回放，原系统相对残差最大约 3.36e-14。独立 `cargo test` 三项通过，release 构建完成。默认 probe 拒绝 llvmpipe；显式软件 smoke 的 3×3 和 514 阶已知解通过，零 RHS 无迭代；受限两步真实系统返回 iteration_limit／退出 2。范围限定只读代码审阅未发现阻断问题。详细命令、原始记录和仍待物理 GPU／消费级设备执行的项目见实验 README 与 validation。
