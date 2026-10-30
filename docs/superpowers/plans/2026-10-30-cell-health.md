# Cell Health Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan in this session. User requested completing the whole authorized change without another planning checkpoint.

**Goal:** Add red-cell health, a minimal player melee attack, and soft deflated corpses through the existing Rust/Godot path.

**Architecture:** Rust owns cell records and combat/lifetime state. Rapier keeps body state and existing fluid interfaces; Godot reads additive display metadata and sends attack intent.

**Tech Stack:** Existing Rust, Rapier 0.36.1, Godot 4.7/GDExtension; no new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-30-cell-health-design.md`

## Global Constraints

- Current branch/workspace; no cloud, solver, mechanical-substep or particle-count changes.
- Only red cells, health, radial melee and corpse softening/deflation/cleanup.
- No automated UI interaction; existing checks and screenshots only.
- Tests target new critical behavior, reuse the existing population fixture, and exercise real production state.
- Existing probe inputs and contour snapshots stay compatible; ordinary far-field identity remains abstract.

## Review Focus

- A pulse damages each intersecting living cell once; held input obeys simulation-time cooldown and walls block damage.
- Dead cells remain physical and advected until cleanup; target area decreases without directly teleporting particles.
- Dead cells never become far-field live stock; total detailed-plus-abstract quantity remains consistent.
- Pause, single-step and reset apply to health, attack feedback and corpse timers.
- Default and optional fluid paths continue to consume all present soft bodies, including corpses.

### Task 1: Complete health, melee and corpse gameplay

**Files:** `crates/simulation/src/cell.rs`, `config.rs`, `runtime.rs`, `runtime/gameplay.rs`, `combat.rs`, `view.rs`, `lib.rs`; `crates/godot_bridge/src/lib.rs`; `godot/world.gd`, `world_cell_mesh.gd`, `world_hud.gd`; README and STATUS.

- [x] Add focused behavior tests for damage/cooldown, wall blocking, real corpse deformation, lifecycle controls and far-field removal, exercising the real simulation.
- [x] Add cell records and corpse material/area interpolation, keeping original soft-body construction and advection.
- [x] Add attack intent, radial boundary intersection and fixed-wall occlusion; coordinate lifetimes and quantity changes at frame boundaries.
- [x] Expose additive cell/attack metadata; draw injury, corpse fading and attack feedback, wire J input, and document controls.
- [x] Run `bash scripts/check.sh`, `cargo test --locked -p simulation --features lbm-gpu`, and existing Godot mesh checks; inspect a normal screenshot if a display is available.
- [x] Review the final diff and record actual verification and remaining manual checks.

Expected: all applicable existing checks and focused gameplay tests pass; actual contour area decreases after death, bodies are removed without reincarnation, and the normal Godot scenes load.

## 实际验证（2026-10-30）

| 命令 / 检查 | 结果 |
| --- | --- |
| `bash scripts/check.sh` | 通过；fmt、workspace check、模拟测试、扩展构建、Godot editor 导入、默认 world 与 lab 短时加载均完成；日志无 SCRIPT ERROR |
| `cargo test --locked -p simulation --features lbm-gpu` | 通过；显式软件 GPU 诊断仍保持 ignored，本轮未运行 |
| `cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu` | 通过，包含既有示例与探针 |
| `DISPLAY=:2 LIBGL_ALWAYS_SOFTWARE=1 godot --path godot --audio-driver Dummy --script res://tests/world_cell_mesh_test.gd` | OpenGL/llvmpipe 下通过，无断言或脚本错误 |
| 正常 world 第 120 步截图 | 已查看；新增 J 提示及尸体统计正常，未注入键鼠输入，未自动运行 UI 交互流程 |
| `git diff --check`、独立只读代码审阅 | 通过；未发现需要修复的问题 |

新增真实模拟测试覆盖了实际轮廓边缘的范围伤害、冷却与距离、固定墙遮挡、尸体面积确实减小且身体保留、到期删除、离开近场不存回活细胞池，以及暂停/单步/重置。首次接入编译中的 Rust 模块可见性和测试借用冲突已修正。

绘制用例最初误用 headless dummy renderer，GPU 缓冲读取断言失败（该脚本仍打印 PASS 且返回 0，不能以退出码代替检查日志）；改用其要求的真实 OpenGL 上下文后通过，没有放宽测试或修改断言。

尚未验证：人工攻击手感、尸体观感与极端拥挤形变、LBM 下新战斗行为的实卡运行和性能。普通活细胞退休后的伤势不保留，遵循既有数量抽象；攻击遮挡使用最近轮廓点的固定墙射线，属于本次最小规则。未新增云实验、优化、撕裂或碎片实现。用户随后要求将本阶段与白细胞实现一并提交推送，归档分支为 `feat/godot-static-background`，不包含合并 `main`。
