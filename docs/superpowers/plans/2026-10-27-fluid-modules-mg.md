# 流体模块化与分块 GPU 多重网格实施计划

> 主 agent 按 `executing-plans` 连续实施，用户已确认路线；只安排范围限定的只读审阅，收尾时用户已明确要求整理文档并提交推送。

**Goal:** CPU LU、旧 GPU GMRES、新 CPU／GPU FGMRES-MG 可独立选择和运行，共享实际问题与原方程审核。

**Architecture:** 三个 crate：`crates/fluid`、`crates/fluid-gpu`、`crates/fluid-bench`。simulation 负责离散／耦合，通过显式工厂接入后端；GPU 依赖不进入默认游戏路径。

**Tech Stack:** Rust 1.94、既有 faer 0.24.4、wgpu 27.0.1、WGSL，复用现有锁定版本。

**Spec:** `docs/superpowers/specs/2026-10-27-fluid-modules-mg-design.md`。

## 全局约束

- 保留自然牵引、原 K 和耦合，默认 CPU LU；不实施 LBM、泵驱动、地图曲线修复或动态域调度。
- 既有实验档案和来源快照保持原样；新数据与新版本来源单独保存。
- 本机没有物理 GPU，不将软件运行时间当硬件性能。
- 保留已有检查；新增测试仅覆盖真实新接口、MG 连通性／传递和 FGMRES 的独立关键行为。

## 步骤

- [x] **公共问题与 CPU 后端。** 创建 `fluid/src/{lib,matrix,problem,cpu,krylov}.rs`，迁移现有 LU 实现，定义工厂／准备后求解接口；用原真实系统和已知不定系统验证。
- [x] **simulation 接入。** 修改 `fluid/{grid,mod,regions}.rs` 与 runtime，传入工厂及固定域参数，默认行为不变；运行既有流体、交接、时钟重置检查。
- [x] **CPU MG／FGMRES。** 创建 `fluid/src/{multigrid,block}.rs`，连通聚合／平滑 P／Galerkin／V-cycle 和右分块预条件；验证粗层连接与实际 small64／wide96／wide192 的原方程残差。
- [x] **GPU 模块。** 抽取旧 baseline，增加设备侧块预条件、MG 与 FGMRES；静态参数／批量提交／状态回读，分别记录迭代、dispatch、提交和回读。
- [x] **统一入口。** `fluid-bench` 提供 backend 选择、capture／replay／probe／smoke；更新 Windows 入口和云端说明，旧档案不重写。
- [x] **集中验证与交付。** CPU LU 等价、CPU MG 收敛、GPU shader／软件功能、新 CLI、项目既有检查、只读审阅；文档区分已验证结果与待物理 GPU 实测项。

## 主要检查点

1. CPU 抽取不能改变原方程／默认行为；以真实矩阵解及保留检查核对。
2. 粗层聚合不把几何上邻近但不连通的流体面混为一簇；测试实际连接关系。
3. FGMRES 更新必须用预条件后的 Z，不可误用 V；已知非平凡预条件系统验证。
4. GPU 参数必须在一个批次内保持各 dispatch 的独立值；shader／软件解与 CPU 对照。
5. 预算耗尽、软件适配器和原方程残差失败必须仍然明确报告失败。

## 进度与实测

实现与集中验证完成，详细结论和原始数据见 `docs/research/experiments/2026-10-27-modular-mg/README.md`。

- 六域共 45 个真实 RHS：CPU LU、CPU FGMRES-MG 的 1／2 次 V-cycle 配置及软件 GPU 的 2 次配置全部通过原方程 1e-4 检查。两次配置为 15–49 次迭代；一次 CPU 配置为 17–57 次。
- 新原生采集入口三个规模各运行 240 帧，矩阵／RHS／LU 解的 108 个二进制文件与旧数据逐字节一致。
- 项目 `scripts/check.sh`、新流体模块测试、旧 lab 兼容测试、默认依赖树检查通过；只读审阅的问题已修复并验证。
- 默认拒绝软件适配器，2 步预算耗尽返回失败，所选场景／区域缺失会明确报错。
- 本机只有 llvmpipe，未验证物理 GPU 性能、Windows 包装脚本或完整 GPU 游戏耦合成本；CPU LU 仍为默认局部后端。
