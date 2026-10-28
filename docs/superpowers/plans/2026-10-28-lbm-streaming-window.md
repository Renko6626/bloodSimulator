# LBM Dynamic Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 将已验证的 GPU D2Q9 MRT＋Guo LBM 从独立研究原型接入正式 Rust simulation，使玩家附近维持一个可按 16×16 区块移动的 3×3、48×48 流体窗口，同时由全局解析血管网络继续提供背景流量。

**Architecture:** 全局 `GraphNetwork` 保持流量权威；正式 LBM 窗口保存局部分布函数、地图固壁和开口边界状态，玩家跨区块时复用新旧窗口的整数格重叠区域，仅初始化新暴露条带。Rapier 继续持有所有实体，simulation 通过一个明确的流体采样/反馈接口在局部 LBM、解析背景和混合带之间切换。第一阶段只做单向局部耦合，不把局部阻力反馈到全局网络。

**Tech Stack:** Rust 2024、wgpu 27.0.1/WGSL、D2Q9 正交原始矩 MRT、Guo 外力、Rapier 0.36.1、现有 `simulation`/`godot_bridge` 分工；GPU 运行失败时必须有明确的禁用/诊断路径，不用软件适配器作性能结论。

**Spec:** `docs/superpowers/specs/2026-10-28-lbm-streaming-window-design.md`

## Global Constraints

- 世界逻辑区块固定为 16×16 游戏单位，LBM 活跃窗口默认 3×3、48×48。
- `Fine` 为 4 格/单位、192×192；`Coarse` 为 2 格/单位、96×96；精度只在窗口创建或 reset 时选择。
- 全局网络提供开口目标流量和背景剖面；局部 LBM 不反馈节点压力、段流量或远处速度。
- 窗口边界按地图几何区分固壁和血管开口；不同开口不得周期回绕连接。
- Rapier 是实体状态唯一所有者；窗口迁移不得重建或搬迁 body。
- 细胞当前四点有限阻力模型和允许透流的近似保留；不顺带实现不可渗透膜、排液或全局拥堵反馈。
- 4/2 格配置、缓冲长度、混合带宽、迁移瞬态与开口公式必须由有针对性的实验确认。
- 不把 `docs/research` 作为正式 crate 依赖；不新增可插拔求解器框架、移动窗口恢复队列或全地图 LBM 状态。

## Review Focus

- 开口切在直管缓冲段时，目标积分通量应保持且不能被每步解析重置覆盖；测试：直管入口/出口的 signed flux、质量和速度误差。
- 固壁与开口同时存在时，窗口不能把血管外边缘当流体或把不同开口周期连接；测试：单分叉 mask 的壁面零通量和每个开口独立通量。
- 窗口跨区块后，重叠分布必须按整数格平移，新条带必须按当前位置的网络流量初始化；测试：迁移前后重叠场误差、中心区速度/密度连续性。
- Rapier body 跨越 LBM/解析区域时不能被迁移或额外施加一次冲量；测试：body handle/位置/速度保持，混合带反馈连续。
- `Fine` 和 `Coarse` 必须使用同一世界坐标与边界契约，不能把格点数硬编码进 runtime；测试：两档窗口 bounds、开口位置和采样坐标一致。

## File Map and Boundary Decision

当前正式工程只有 `crates/fluid-gpu` 的 Stokes GPU 后端，研究 LBM 位于 `docs/research/2026-10-27-ns-lbm/lbm`。实现前必须在 Task 1 通过短设计记录确认正式归属：推荐扩展现有 `crates/fluid-gpu`，新增独立 `lbm` 模块并保留现有 Stokes factory；`simulation` 只依赖一个窄的本地流体窗口接口，Godot 不直接调用 wgpu。若性能/依赖隔离要求不允许，则改为新 `crates/fluid-lbm-gpu`，但必须保持同一 simulation-facing API。计划后续任务使用以下逻辑边界名，实际 crate 名在 Task 1 记录后固定：

- `fluid-gpu` 或 `fluid-lbm-gpu`：GPU context、LBM buffers/shaders、窗口状态迁移、边界 kernel、GPU/CPU 诊断。
- `crates/simulation/src/fluid`：地图到网格的 mask/opening 描述、全局背景采样、混合权重、Rapier 耦合，不拥有 wgpu 类型。
- `crates/simulation/src/runtime.rs`：区块窗口原点、迁移时机、唯一 Rapier step 和每子步采样顺序。
- `crates/simulation/src/config.rs`/`view.rs`：精度档、窗口诊断和可读快照。
- `crates/godot_bridge/src/lib.rs`：只转换配置/快照，不持有流体状态。
- `docs/research`：实验脚本和结果归档，不作为生产运行时依赖。

### Task 1: Confirm Production GPU Boundary and Extract the LBM Contract

**Files:**
- Create: `docs/superpowers/specs/2026-10-28-lbm-production-boundary-design.md`
- Modify: `Cargo.toml`, selected GPU crate `Cargo.toml`, `crates/simulation/Cargo.toml` only if the approved boundary requires it
- Test: existing workspace compile plus a contract-level Rust test in the selected crate

**Interfaces:**
- Consumes: `docs/superpowers/specs/2026-10-28-lbm-streaming-window-design.md`, research `model.rs`/`lbm.wgsl` behavior.
- Produces: `GridProfile::{Fine,Coarse}`, `WindowOrigin`, `WindowSpec`, `OpeningSpec`, `FluidSample`, and a crate boundary that later tasks can implement without importing research paths.

- [x] **Step 1: Write the boundary decision and contract test**
  - Pin the approved crate owner, feature/default behavior, GPU initialization error behavior, and the ownership of `GridProfile`, `WindowOrigin`, and opening metadata.
  - Test that Fine/Coarse map to 48×48 world bounds and 192×192/96×96 cells, and that shifting one logical block produces 64/32 lattice-cell offsets.
- [x] **Step 2: Run the focused contract test and workspace check**
  - Run `cargo test --offline --locked -p <selected-crate> <contract-test>` and `cargo check --offline --locked -p simulation`.
  - Expected: the new test fails before implementation, then passes after the minimal contract types are added.
- [x] **Step 3: Record the boundary and dependency decision**
  - Document why the selected crate preserves existing Stokes ownership, how simulation calls it, and how software adapters are restricted to functional checks.
- [x] **Step 4: Run `git diff --check`**

### Task 2: Build the Production LBM Window Core

**Files:**
- Create/Modify: selected GPU crate `src/lbm/`, `src/lbm/model.rs`, `src/lbm/lbm.wgsl`, `src/lbm/gpu.rs`
- Modify: selected GPU crate public `lib.rs`
- Test: selected crate LBM model/GPU smoke tests

**Interfaces:**
- Consumes: `WindowSpec`, `GridProfile`, `OpeningSpec`, wall/opening masks, background initialization fields.
- Produces: `LbmWindow::new(spec, initial, geometry)`, `advance(step_input)`, `sample(world_position)`, `snapshot_diagnostics()`, and `shift_to(new_origin, initial_strip)`; no Rapier or Godot types.

- [x] **Step 1: Add failing tests for model invariants**
  - Test shifted D2Q9 populations preserve finite positive density on a uniform background, zero-force equilibrium remains stationary, and Fine/Coarse coordinate transforms agree in world space within the documented tolerance.
- [x] **Step 2: Port the existing MRT/Guo core without changing its equations**
  - Preserve shifted populations `f_i-w_i`, the current orthogonal raw-moment matrix, conservation-mode relaxation rates, half-force velocity, and Guo `(I-S/2)` source treatment.
  - Keep `f32` GPU state and a CPU f64 reference only for diagnostics/tests; do not copy the research crate into production by path.
- [x] **Step 3: Add persistent window metadata and integer-overlap shift**
  - Store origin, profile, dimensions, mask, opening descriptors, ping-pong populations, and macro output together.
  - Implement overlap copy by lattice-cell offset; initialize only exposed strips from the supplied background state.
- [x] **Step 4: Add focused GPU/software smoke coverage**
  - Use the existing `--allow-software` style only for finite-state/coordinate checks; reject software adapters for performance commands.
- [x] **Step 5: Run focused tests and formatting**
  - Run `cargo test --offline --locked -p <selected-crate> lbm`, `cargo fmt --check`, and `git diff --check`.

### Task 3: Implement Geometry-Derived Walls and Fixed-Flow Openings

**Files:**
- Modify: `crates/simulation/src/fluid/` (new focused geometry/opening module; keep current Stokes modules intact)
- Modify: selected GPU crate boundary shader and its Rust upload types
- Test: `crates/simulation/src/fluid/tests.rs` or a focused new test module

**Interfaces:**
- Consumes: `MapDefinition`, `GraphFlow`, `WindowSpec`, `GridProfile`.
- Produces: `WindowGeometry { mask, openings, background_field }`, with each opening carrying world endpoints, outward normal, signed target flux, tangent, and background profile parameters.

- [x] **Step 1: Write failing geometry tests**
  - For a straight tube, assert two independent openings with opposite signed flux and no fluid support outside the tube.
  - For a single fork, assert branch openings are separate and signed fluxes sum to zero; assert non-opening perimeter cells are wall cells.
- [x] **Step 2: Implement map-to-window rasterization**
  - Derive fluid mask from the existing shared map geometry, preserving world coordinates and avoiding a second wall definition.
  - Reject a connected fluid window that has no usable opening instead of silently adding a periodic edge.
- [x] **Step 3: Implement background initialization**
  - Convert GraphFlow’s segment flow and tangent into the chosen 2D background profile for the window’s grid spacing; keep the circular-network/2D-grid mapping explicit and testable.
- [x] **Step 4: Implement open-boundary reconstruction**
  - Apply target integral flux through unknown incoming populations while allowing the interior LBM to evolve; never rewrite the complete opening with equilibrium every step.
  - Keep mass/flux correction local to the opening and report residuals.
- [x] **Step 5: Run numerical checks**
  - Run the straight-tube analytic profile, zero-pressure/zero-flow, and fork signed-flux tests. Expected: finite density, wall no-through-flow, opening flux within the selected tolerance, and no periodic cross-opening transport.

### Task 4: Add Window Movement and Background Handoff

**Files:**
- Modify: `crates/simulation/src/fluid/regions.rs` or a new `streaming_window.rs`
- Modify: selected GPU crate `LbmWindow::shift_to`
- Test: focused streaming-window tests

**Interfaces:**
- Consumes: `WindowGeometry`, `LbmWindow`, `WindowOrigin`, `GraphFlow`.
- Produces: `StreamingWindow::{active_origin, bounds, sample, begin_frame, maybe_shift}` and migration diagnostics (`region_switches`, overlap error, center continuity, strip warmup state).

- [x] **Step 1: Write failing shift tests**
  - Start from a deterministic spatial population field, shift one block in x/y, assert overlap values are copied at the exact integer offset and exposed strips are initialized from the current background flow.
  - Assert no second shift occurs in one physics step and that origin stays block-aligned.
- [x] **Step 2: Implement overlap reuse**
  - Rebuild only metadata/mask/openings for the new origin; preserve the old state in the overlap; clear/init the exposed strip.
- [x] **Step 3: Add warmup and diagnostic capture**
  - Record center-region velocity/density/momentum deltas for a bounded number of steps after migration; do not add an automatic recovery queue.
- [x] **Step 4: Validate fixed straight-tube and fork migrations**
  - Run a short trajectory with one block shift and compare the center region against an unshifted reference over the same physical time. Report, rather than hide, any transient above the chosen tolerance.

### Task 5: Integrate Rapier Sampling, Mixing, and Single-Step Ordering

**Files:**
- Modify: `crates/simulation/src/fluid/mod.rs` and focused coupling module
- Modify: `crates/simulation/src/runtime.rs`
- Modify: `crates/simulation/src/config.rs`
- Test: `crates/simulation/src/fluid/tests.rs`, `crates/simulation/src/runtime.rs` tests

**Interfaces:**
- Consumes: `StreamingWindow`, `GraphFlow`, Rapier world/body handles.
- Produces: `FluidSample { velocity, local_weight, valid }`, `LocalFluid::sample_world`, and runtime config for grid profile/window size/transition width.

- [x] **Step 1: Write failing runtime/coupling tests**
  - Assert a cell and robot keep the same Rapier handles across a window move.
  - Assert outside-window sampling uses analytic velocity, inside sampling uses LBM, and the transition band is continuous and scales local feedback once.
  - Assert `prepare -> drive/advect -> LBM advance -> Rapier step` remains the only body-step order and no local preparation moves a body.
- [x] **Step 2: Implement world-coordinate sampling**
  - Convert markers through the active window origin/profile; sample local macro velocity only where the geometry says fluid support exists.
  - Use analytic GraphFlow sampling outside support and a compact smooth transition weight in the configured outer band.
- [x] **Step 3: Integrate cell and robot feedback**
  - Keep Rapier handles and world coordinates unchanged; apply local LBM reaction only with the local weight, and analytic cell driving only with the complementary weight.
  - Preserve the current four-marker permeable model and explicit one-way global-flow decision.
- [x] **Step 4: Integrate block-aligned migration into `Simulation::step_inner`**
  - Trigger at most one migration at a fixed-step boundary after the player crosses the selected block threshold; update the active window before the next substep.
- [x] **Step 5: Run focused simulation tests**
  - Run `cargo test --offline --locked -p simulation fluid:: runtime::` (or exact filtered tests), then `cargo fmt --check` and `git diff --check`.

### Task 6: Add Configuration, Snapshot Diagnostics, and Explicit GPU Runtime Selection

**Files:**
- Modify: `crates/simulation/src/config.rs`, `crates/simulation/src/view.rs`, `crates/simulation/src/runtime.rs`
- Modify: `crates/godot_bridge/src/lib.rs`
- Modify: selected crate feature/config wiring and `Cargo.toml` only as approved in Task 1
- Test: bridge/config serialization or existing simulation integration tests

**Interfaces:**
- Consumes: `GridProfile`, migration diagnostics, local field stats.
- Produces: reset-time profile selection, snapshot fields for active bounds/profile/migration/flux diagnostics, and a clear runtime error or fallback when no usable GPU adapter exists.

- [x] **Step 1: Add failing configuration/snapshot tests**
  - Assert reset preserves the selected `Fine`/`Coarse` profile and snapshot reports the active bounds/profile without exposing wgpu handles.
  - Assert software-adapter policy is distinguishable between functional checks and performance/runtime selection.
- [x] **Step 2: Implement config and snapshot plumbing**
  - Keep the first user-facing controls explicit and reset-scoped; do not add automatic runtime LOD or UI-specific solver state.
- [x] **Step 3: Keep Godot bridge data-only**
  - Convert profile, bounds, active opening/flux diagnostics and migration counters only; Godot must not advance or own LBM state.
- [x] **Step 4: Run existing bridge and simulation checks**
  - Run `cargo test --offline --locked -p simulation`, `cargo check --offline --locked -p godot_bridge`, and the project’s existing `scripts/check.sh` if available.

### Task 7: Run Parameter Experiments and Decide the First Shipping Profile

本轮 RTX 3060 实卡已经执行：10 个案例全部在完成前失败，没有迁移发生，不能给出有效完整性能 p95 或首发 profile。全新独立 Sol high 调查确认当前低 lattice 黏度下，固定速度出口存在增长的横向边界模；已保留纯流体复现、单变量控制和定量模式注入证据，生产求解器未改。见 [实卡与独立调查记录](../../research/2026-10-28-lbm-streaming-window/hardware-rtx3060-20261028/README.md)。后续须先确认稳定出口方案，再恢复未通过的性能／配置验收。

**Files:**
- Create: a focused experiment entry under `crates/simulation/examples/` or the approved existing probe location
- Create: result CSV/JSON and a short report under `docs/research/` (do not overwrite prior LBM results)
- Modify: no production code unless an experiment exposes a concrete bug

**Interfaces:**
- Consumes: the production window API, representative straight-tube/fork geometry, 0/representative cells, Fine/Coarse profiles.
- Produces: measured values for buffer sensitivity, transition width, migration transient, strip warmup, GPU/Rapier/full-step p95, and a recommendation for initial profile.

- [x] **Step 1: Define the minimal experiment matrix**
  - Fixed 48×48 straight tube: no cells and representative cells, Fine/Coarse.
  - One ordinary fork: same profiles and cell load.
  - One block migration: compare center-region speed/density/momentum and cell force before/after migration.
  - Vary only the buffer/transition values needed to choose a first configuration; do not expand to full-map or special chambers.
- [ ] **Step 2: Run numerical and performance measurements**
  - Count all lattice steps, upload/encode/wait/readback, coupling, Rapier, and full-step p95. Use software only for functional checks and a physical GPU for performance.
  - 已执行源码 `7727c60` 的 RTX 3060 矩阵，取得 303 行失败前完整帧记录；10／10 案例失败，有效完整性能测量仍被出口稳定性阻塞。
- [x] **Step 3: Analyze and record the decision**
  - State whether Fine is viable, whether Coarse is the fallback, whether 16-unit buffering and the tested transition are adequate, and which cases remain unsupported.
  - 本轮决定：当前 Fine、Coarse 均未通过，不选取首发 profile，也不认定缓冲或混合带已验收；固定密度出口仅作因果控制，不能未经确认改变各开口精确流量契约。
- [x] **Step 4: Run final relevant checks**
  - Re-run only the focused tests affected by any concrete repairs, plus `git diff --check`; do not claim full-game acceptance from research probes.
  - 本次生产数值代码没有改动；核对原始 18 文件与源码／二进制哈希、归档独立复现源码和日志、检查文档差异与链接。实例已确认 stopped／exited。

## Self-Review

- The spec’s global/local split is covered by Tasks 1, 3, 5, and 6; no task sends local resistance back into `GraphNetwork`.
- Both grid profiles and reset-only switching are covered by Tasks 1, 2, and 6; no runtime projection/LOD task is included.
- Geometry-derived walls/openings and fixed-flow semantics are covered by Task 3, including the explicit prohibition on periodic cross-opening transport.
- Overlap reuse, exposed-strip initialization, and migration diagnostics are covered by Task 4.
- Rapier continuity and mixed local/analytic sampling are covered by Task 5.
- Experimental buffer/migration/profile selection is intentionally separated into Task 7 so parameters are not guessed in production code.
- The remaining major implementation decision is the formal GPU crate boundary in Task 1; it is recorded as a required design decision because the current project keeps the research LBM outside the production workspace and the existing GPU crate currently owns a different solver.

