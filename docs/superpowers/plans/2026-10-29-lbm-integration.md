# LBM 可选路径接入计划

> 执行：沿用已获授权的 Sol high 实验子代理完成代码，主代理完成文档与集成核验。使用 subagent-driven-development；项目关于验证范围和权限的约定优先。

**目标：** 将已选 LSQ-FD 边界和帧末平均密度控制纳入正式可选模块，保存可核对的物理说明。

**架构：** 保持 blood-fluid-gpu → simulation → Godot 的现有职责；后端持有数值算法，simulation 明确帧尾时序。默认关闭 lbm-gpu，默认解析流不改。

**技术栈：** Rust、wgpu/WGSL，复用 workspace faer 0.24.4。

**已确认方案：** [接入准备方案](../specs/2026-10-29-lbm-integration-preparation.md)。分支 feat/lbm-lsq-integration，起点 f0ff276。

## 约束与审阅重点

- 不改研究存档，不引入实验模式或外部权重文件依赖；不自动开云实例。
- LSQ 半径 4、二次拟合、距离权 1/(1+r²)、SVD rcond=1e-12、rank=5、condition≤1000；取样不可穿墙，不加回退算法。
- 创建/迁移先验证新几何与权重，再修改 GPU 状态；速度剖面改变复用权重。
- 帧末只控制一次，中间子步继续刷新采样；保留每子步 prepare。检查延迟读取是否影响机械计算。
- 补偿质量/动量与机械反作用分开；保留原耦合有效断言。small_world 等几何限制不放宽。

## 任务 1：正式数值模块与调用时序

所有权：子代理负责 crates/fluid-gpu、crates/simulation、必要 Cargo.lock 更新；不修改主代理文档。

- [x] 新增私有 stencil 模块，运行时生成实际 GPU 使用的权重，new/shift_to 更新资源，set_opening_profiles 复用。
- [x] 正式 WGSL 接入固定 LSQ-FD 全九分布重建及单独均值修正 pass，移除对研究开关的需求。
- [x] 后端提供显式均值控制及诊断，simulation 在最后 batch 延迟读回、finish_frame 修正并刷新缓存；不扩展 Godot/snapshot。
- [x] 最小关键验证：真实权重多项式重现、退化/穿墙；GPU 密度与 Guo 矩；一帧一次、缓存及移窗。运行受影响已有 LBM 测试；旧“已知分布不改写”断言改为全九重建的真实行为断言。

## 任务 2：物理文档与交付核验

所有权：主代理负责 docs/physics/lbm-fluid.md、项目文档入口与状态记录。

- [x] 说明全局图/局部二维流、D2Q9 MRT+Guo、单位换算、给定速度密度闭合、LSQ-FD 应力重建、帧尾均值控制与非守恒补偿。
- [x] 区分已验证单例与未验收范围，引用研究数据与原始文献。
- [x] 核对实际代码与文档；集中检查 feature 开/关编译及必要现有测试，避免重复全量验证。
- [x] 记录命令、结果与未验证项，保留分支供正式接入，不自动合并或推送。

## 执行记录

- 2026-10-29：用户确认推荐范围；复用已授权子代理开始任务 1，主代理开始任务 2。

- 2026-10-29：任务 1、2 完成。已实现运行时 SVD stencil、正式 LSQ-FD 全九分布重建、独立均值修正 pass、请求质量/动量补偿诊断、严格帧末调用与缓存刷新。仅增加已有 faer 的直接依赖；未引入新版本、Godot 选项或外部权重文件。
- 独立只读审阅逐项比对冻结 Python/WGSL 候选和实际帧调用链，未发现关键或重要问题；文档状态与验证记录的收尾项已补齐。碰撞、stream、macros、kicks shader 数值文本保持不变，研究源码与原始数据未改。

## 实际验证

以下在本机执行。Cargo 使用 `CARGO_TARGET_DIR=/data/sunyunbo/www/bloodSimulator/target` 复用依赖缓存；子代理测试另设 `CARGO_BUILD_JOBS=2`。软件 GPU 为 llvmpipe（LLVM 15），不据此报告硬件性能。

| 命令 | 结果 |
| --- | --- |
| `cargo test --offline --locked -p blood-fluid-gpu lbm --lib` | 4 passed，6 个显式 GPU 诊断 ignored |
| `cargo test --offline --locked -p blood-fluid-gpu lbm --lib -- --ignored --test-threads=1 --nocapture` | 6 passed，含原有边界/迁移/冲量及新增 Guo 均值矩检查 |
| `cargo test --offline --locked -p simulation --features lbm-gpu --lib lbm_geometry` | 3 passed，1 个 GPU 诊断 ignored |
| `cargo test --offline --locked -p simulation --features lbm-gpu --lib -- --ignored --test-threads=1 --nocapture` | 8 passed，含帧循环、缓存、迁移、机械冲量和默认地图明确拒绝 |
| `cargo test --offline --locked -p blood-fluid-gpu stencil --lib` | 最终加强四法向线性/二次向量场与退化夹具后，2 passed |
| `cargo test --offline --locked -p blood-fluid-gpu lbm_shift_xy_preserves_populations_and_initializes_changed_strip --lib -- --ignored --test-threads=1` | 增加不支持新 stencil 时保持原状态的断言后，1 passed |
| `cargo check --locked -p simulation -p godot_bridge --no-default-features` | exit 0 |
| `cargo check --locked -p simulation -p godot_bridge --features godot_bridge/lbm-gpu` | exit 0 |
| `cargo fmt --all -- --check`、`git diff --check` | exit 0 |
| 七份新增/更新入口文档的本地链接检查 | 无缺失目标 |

新检查验证真实生产路径：权重不穿墙及退化拒绝；均值修正的逐格密度、速度、Guo 修正二阶矩、质量/动量变化与固体排除；移窗重绑后继续推进；正常两个机械子步帧总共 3 次读回（field、raw、field）。旧机械反作用断言保持。旧“六个已知分布完全不改写”与全九重建冲突，已替换为 streaming 后密度闭合及实际重建检查，保留目标速度/流量等有效断言。

均值控制返回的是请求补偿量，未扩大 snapshot，也未用推算值冒充每帧实测守恒残差。GPU 检查用实际修正前后 raw 分布独立观测矩；正式运行保持既定读回预算。

未执行：新 RTX 实卡长测、完整 Godot 游戏/UI 交互、完整耦合精度、长期移窗、任意角度与绝对压力响应。原实卡 2.10 ms p95 与 10000 帧结果属于冻结的纯流体实验。代码保留在接入分支，不自动合并或推送，云实例未启动。
