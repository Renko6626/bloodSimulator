# 固定局部窗口接入实施计划

> 使用 superpowers:executing-plans 在当前 `feat/engine-foundation` 内执行；用户已明确同意本方案试接，不重复询问执行方式。

**Goal:** 在现有 Godot 世界接一个可观察、可测量的固定 64² Stokes 分叉窗口。

**Architecture:** simulation 内部 fluid 模块构建真实地图的阶梯 MAC 域，带状 Cholesky 复用速度逆，原压力 Schur-CG 求局部不可压场。runtime 统一施加受力和推进 Rapier；bridge/Godot 仅展示。

**Tech Stack:** Rust，Rapier 0.36.1，Rayon 1.12.0，既有 Godot/GDExtension。

**Spec:** [已确认设计](../specs/2026-10-26-fixed-fluid-window-design.md)。用户已确认速度逆替换和自然牵引开口。

## Global Constraints

- 现有分支原地继续，保留之前尚未提交的研究与文档；不自动提交或推送。
- 固定中心 (-14,0)、边长 16、64²；不做移动窗口、LOD、全局堵塞或新外部求解库。
- 保留唯一 Rust/Rapier 步进，每机械子步更新一次流体。
- 窗口内外分摊驱动，不同时施加两套完整拖曳；弱反馈强度从原研究值开始。
- 新测试仅覆盖真实新增物理行为；UI 只看截图，不自动操作。

## Review Focus

- 开放压力参考与自然牵引、速度半控制体权重须一致；直管流量与外压平移验证。
- 固壁与孤立封闭域不能悄悄漏流或产生未处理压力零空间。
- 细胞进入/退出、机器人驱动和反冲量顺序不能重复施力。
- 重置须保留启用局部窗口的场景；线程池跨重置复用。
- 实测若成本不适合试用，按设计停在数值原型，不能自动更换路线。

## Task 1: 固定几何求解器

**Files:** `crates/simulation/src/fluid/{mod,linear,grid,tests}.rs`，`src/lib.rs`。

**Interfaces:** `Grid::new(n, spacing, origin, mask, viscosity)`；开口外压和节点体力求 `Field`，返回速度、压力与真残差；局部场通过世界坐标采样。

- [x] 写并运行直管解析流量、外压平移、封闭域拒绝的简短生产逻辑检查。
- [x] 实现带状速度分解、同权重 D/G、开口压力载荷、压力 CG；检查通过。
- [x] 用真实分叉地图验证开口净流量与初始化、求解时间；性能不合适时按设计报告。

## Task 2: 世界与受力接入

**Files:** `fluid/mod.rs` 及必要耦合文件，`runtime.rs`、`cell.rs`、`view.rs`、workspace 依赖和现有探针。

**Interfaces:** 固定窗口构造、按泵压与物体状态步进、局部速度、反冲量、显示统计。`Simulation::new_world` 保留解析对照，新增显式局部世界构造供 Godot 试用。

- [x] 接入应用生命周期四线程池及 Rapier parallel，不在 reset 重建全局池。
- [x] 接入单位泵压背景场、逐细胞四标记、机器人、窗口空间过渡与每子步冲量。
- [x] 复用暂停/单步/重置检查增加局部场景，执行短时真实探针并保存成本与残差。

## Task 3: Godot 展示与交付

性能停点已触发：固定核短测整步 59.249 ms，流体 48.336 ms；解析世界对照 8.376 ms。依据已确认设计第 4 条，本任务的 UI 接入暂缓，不自动继续。数值原型已完成一次只读代码审阅；结果见[试接报告](../../research/2026-10-26-fixed-window-trial.md)。

**Files:** `godot_bridge/src/lib.rs`、`godot/world.gd`、`world_hud.gd`、README、STATUS 和实施记录。

- [ ] 现有世界入口启用试验，展示固定窗口边框、局部流向及少量状态；保留原 lab。
- [ ] 集中执行 `bash scripts/check.sh`，截图查看；不自动操作输入。
- [ ] 一次只读最终审阅，处理核心问题，报告实际结果、手感及性能限制。

## 执行记录

计划已自查：任务 2 消费任务 1 的场和采样；任务 3 消费任务 2 的快照，不另建模拟。当前执行由主 agent 内联完成，最终仅做一次范围内审阅。
