# 子步缓存与 1/4/16 细胞实验只读核查

核查时间：2026-10-23。范围：`shared/fluid.rs`、当时的 `rapier/src/main.rs` 与本机 Rapier 0.36.1 源码；未修改产品代码或实验 harness。

## 已确认

- `Fluid::prepare_markers` 只保存每个 marker 的冻结 `position`、`velocity`、`drag` 和 4 点核的网格 stencil。`advance_prepared` 每个 LBM 步仍用当前 `self.f`、上一步 `self.force` 调用 `macrostate` 采样流速、重新计算 drag、网格力、碰撞/传播与 impulse diagnostics。`advance` 只是每步先 prepare 再调用同一个 `advance_prepared`；缓存路径没有改动流体求解公式。参见 `shared/fluid.rs:146-275`。
- harness 每个 Rapier 子步先从全部 body 构造新的 `frozen`，缓存模式在该子步调用一次 `prepare_markers`，然后两模式用同一个冻结数组推进若干 LBM 步，最后把 impulse 映射到节点并调用一次 `world.step()`。因此跨子步不会沿用旧 stencil；即便一个子步没有 LBM 步，下一子步也会重建。参见 `rapier/src/main.rs:140-192`。
- 单个 body 的 4 个 marker 各用 `GAMMA/4 = 0.0625`；4/16 body 共分别 16/64 marker，仍是一个 `Fluid::new(32, 2.0, 0.0)` 和一个 PhysicsWorld，每个 LBM 步按全场 marker 一次求解。每个 marker 的 impulse 以 1/2 均匀分配给 12 个节点、另 1/2 给选定节点，权重总和为 1。参见 `rapier/src/main.rs:10-11,34-45,126,147,163-191`。
- `scale` 的 1/4/16 body 均用半径 0.075、固定单位域 n=32、单体质量 1、同样材料和 gamma。三个参考场景则是单体半径 0.15；两组物理尺寸不同，不能把参考场景单体和 scale 单体作物理等价对照。参考场景只适合验证缓存 A/B 的行为一致性。参见 `rapier/src/main.rs:73-110,126`。
- clock 用累计目标 `floor(t/lb_dt + 1e-10)` 与已运行累计步数相减，消除了每子步独立取整的累计漂移；帧末 mismatch 小于一个 LBM `dt`。`tau=2,n=32` 给 `lb_dt=1/2048`，每个 Rapier 子步约 17.07 个 LBM 步。参见 `shared/fluid.rs:104-107`、`rapier/src/main.rs:146-149,229-231`。

## 需要更正的实验口径

1. 当前 CSV 列名 `rapier_soft_contact_constraints` 与数据来源不符：代码累加 `body.volume_contacts().count()`（`rapier/src/main.rs:202,220`），并不是跨细胞 solver contact 数。Rapier 0.36.1 有公开轻量 API `world.physics_pipeline.counters.solver.ncontacts`，其文档明确包含 soft-body contact points；实现于 `.../rapier2d-0.36.1/src/dynamics/solver/staged_island_solver/init.rs:609` 将 `self.soft_constraints.contacts.len()` 相加。该计数是最近一次 `world.step()` 的**全场** solver 点数，含其他类型接触及 CCD 子步累计。可每个 Rapier 子步后采值，再按帧记录 sum/max；列名和说明要反映口径。若要单独证实 cell-cell，可以检查 `world.narrow_phase.contact_pairs()` 中 `pair.soft()` 的候选或 impulse，但它是候选/接触槽口径，不能与 `ncontacts` 混称。不能用边界 segment 数代替接触数。
2. 当前 `scale` 全部 body 初速度均为 `(0.05,0)`，16 body 4×4 中心为 0.17/0.39/0.61/0.83，半径 0.075，相邻初始几何 gap 约 0.07。相同初速度不会主动形成相向接触。若此场景用于压测 cell-cell 接触，应明确增加相向运动的短场景，或如实将现有结果标为“多体无主动碰撞”规模测试。
3. `closest_intercell_node_distance` 只是任意两体节点间最短距离（`rapier/src/main.rs:221-226`），不是轮廓线段间的最小距离，更不是 solver 接触数。若沿用可作为间接接近证据，但应保持该字段名并说明几何局限。
4. timing 边界：baseline 在 `fluid_ms` 内每步执行 prepare，cached 在 `prepare_ms` 中每 Rapier 子步执行一次。比较缓存收益须用 `prepare_ms+fluid_ms` 或 `whole_ms`；单看 `fluid_ms` 会高估收益。`rapier_ms` 包含施加节点 impulse 与 `world.step()`，`node_transfer_ms` 还包含驱动力/板位置设置；`whole_ms` 在 CSV 写入前采时，排除了文件写入。参见 `rapier/src/main.rs:151-195,235`。
5. A/B 轨迹应按相同 scene/count/duration、相同 frame 与 cell/node ID 对齐比较 `frames.csv` 中中心/速度/面积/流场诊断，及 `outlines.csv` 中逐节点位置；允许 few% 的已授权误差，需同时看有无非有限量、面积翻转或明显跳变。单看末帧或只比运行时间不足以证明缓存未改变运动。

## 接触 API 源码定位

- `PhysicsWorld::physics_pipeline` 为公开字段：`rapier2d-0.36.1/src/pipeline/physics_world.rs:73-77`。
- `PhysicsPipeline::counters` 为公开字段：`rapier2d-0.36.1/src/pipeline/physics_pipeline/mod.rs:47`。
- `SolverCounters::ncontacts` 注释说明含 soft-body contact points：`rapier2d-0.36.1/src/counters/solver_counters.rs:9-13`。
- `NarrowPhase::contact_pairs()`、`ContactPair::soft()` 公开：`rapier2d-0.36.1/src/geometry/narrow_phase/queries.rs:198-200`、`src/geometry/contact_pair.rs:475-482`。


归档补记：以上记录对应实施中的审查版本。最终 harness 已使用 `solver_contacts_sum/max`，增加独立 counterflow 场景；scale 同向漂移仍不作为碰撞证据。时间步手算已按源码公式更正。最终数据与严格 A/B 对照见父目录 README 和专题报告。
