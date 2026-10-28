# Whole implementation review

Reviewed range: `7d3d33444d969320d8575225dc1e1645852ec41c..7727c605c6000dd0e3ad6cbf413b3b17f4f3c1c4` on `feat/engine-foundation`.

Reviewer: Sol high. Production files, index and HEAD were read-only. No subagents, browser interaction, cloud activity, production edits or repeated test suites were used.

## Verdict

- **Requirements: PASS for Tasks 1–6 and Task 7 local preparation.** Task 7 hardware measurement, parameter assessment and shipping-profile decision remain incomplete, as the plan records.
- **Code quality: APPROVE WITH MINORS for the reviewed implementation and bounded external measurement.** No new Critical or Important defect found. The two previously deferred Minor findings remain nonblocking at the current map scale and entry flow.
- This is permission to regard the code review gate as satisfied, not evidence that Fine, Coarse, the default game map or a shipping configuration has passed hardware/game acceptance.

## Findings

### Critical

None found in this range.

### Important

None found in this range. The known representative-pressure Fine/fork numerical failure is a preserved negative experiment, not an unreported supported case. Its cause remains unknown and it prevents accepting that configuration. Task 7 explicitly permits reporting unsupported cases; this review does not request an unapproved numerical-method change.

### Minor — deferred extreme-coordinate overflow

`crates/simulation/src/fluid/streaming_window.rs:35–43` (`origin_for`, particularly line 42).

The f32 lower-bound expression rounds the intended `i32::MIN + 2` margin back to `i32::MIN`. A finite coordinate of approximately `-3.436e10` world units can therefore pass validation, convert to `i32::MIN`, and overflow when subtracting one for the window origin. Debug builds panic; release arithmetic can wrap. The same issue applies to y.

This is the already recorded Task 4 minor. Current maps use coordinates on the order of tens or hundreds of units, so it does not block their operation or this hardware matrix. If addressed later, use a correctly represented bound or checked integer subtraction in this small helper. It does not justify a larger validation/recovery mechanism or additional test framework.

### Minor — deferred stale bridge error after mode replacement

`crates/godot_bridge/src/lib.rs:38–45` (`use_world`, `use_local_world`).

Both methods replace the simulation without clearing `BloodLab.error`. If invoked after an earlier bridge error, the new simulation can run while the snapshot still reports the old error and the error-dependent pause/single-step controls remain inconsistent. The Result-based creation/reset methods do clear the error through `record_result` at line 195.

This is the already recorded Task 6 minor. `godot/world.gd` currently calls these methods on fresh startup, where the error is empty, so there is no current normal-entry blocker. A future in-session mode-selection flow should clear the latch after successful replacement; no new state machine is needed.

## Cross-module requirements assessment

### Production ownership and default path

The dependency and ownership direction matches the approved production-boundary design: optional `simulation/lbm-gpu` depends on `blood-fluid-gpu::lbm`; the GPU crate depends on the shared fluid crate. Simulation does not import research code or wgpu handles. Existing Stokes modules and their factory ownership are preserved. Both simulation and bridge features default to empty; ordinary game construction remains analytic.

`LbmOptions` and explicit LBM constructors carry creation/reset options. Profile changes create a replacement window through reset, rather than projecting live state or introducing automatic LOD. The bridge converts data and delegates stepping/reset to Simulation. Hardware-only adapter selection is the runtime default; software is an explicit diagnostic option and is identified in snapshots/results. Creation failures propagate through Result and the Godot startup path exits without silent analytic substitution.

### Numerical core and opening boundary

The shader preserves shifted D2Q9 populations, the orthogonal raw-moment MRT relaxation, conserved density/momentum modes, Guo source relaxation and half-force macro velocity. The f64 reference remains diagnostic/test-only. The forced-strip initializer carries the retained half-force correction.

Streaming uses explicit finite-window checks and solid-link bounce-back; there is no periodic connection between openings. Reconstruction runs after streaming and writes the three unknown incoming populations only. Its rotated normal/tangent form consistently restores the population offset and half-force momentum. Known outgoing/tangent populations continue to carry local evolution rather than being overwritten with equilibrium.

Map rasterization derives solid support from the shared signed-distance geometry. Openings are individual perimeter intervals, with corners, ambiguous edge ownership and curved local buffers rejected visibly. Curvature away from the local buffer is allowed. The common equivalent depth and planar Poiseuille initialization are explicit; this preserves graph flux while deliberately not reproducing circular mean velocity. The initial nearest-edge junction field is documented as an initial condition, not a divergence-free solution.

Graph flow remains authoritative: `maybe_shift` refreshes profiles even without migration, and migration initialization uses the current flow. Local kicks never alter graph pressures/rates. Opening flux diagnostics use world normal velocity times grid spacing and report residuals against current targets.

### Migration, sampling and feedback

The same profile derives world bounds, spacing and block offsets. GPU migration uploads the initialized destination and then copies the integer-overlap populations exactly. Simulation rebuilds map/opening metadata, caches the resulting macros and records bounded center/overlap observations. Migration touches no Rapier body or handle. The frame guard permits at most one migration per fixed tick even though preparation occurs before each mechanical substep.

Cached sampling interpolates only supported finite fluid cells and normalizes the same weights used for momentum spreading. World-map solid/invalid positions receive no exterior background drive. Fluid points outside the window receive analytic background; the outer band supplies a smooth local weight. Analytic cell driving is scaled by the complementary local weight. Marker exchange applies the local weight once, applies equal opposite fluid momentum through the normalized support, and preserves the four-marker permeable approximation.

Runtime ordering is prepare → robot/cell drive → local LBM exchange/advance → the sole `self.physics.step()` at `crates/simulation/src/runtime.rs:507`. The LBM module does not step a second physics world. Lattice time sums to the mechanical substep; the timing guard rejects incompatible public dt/substeps edits before frame mutation and reset rebuilds the lattice timing. Error propagation explicitly warns that an advance failure can leave a partially advanced frame; the bridge pauses and requires user-directed reset rather than hiding it behind recovery.

### Diagnostics and measurement honesty

Snapshots read the cached field; they do not submit extra GPU work. `advance_batch` now validates the already-read macrostate at `crates/simulation/src/fluid/streaming_window.rs:160` and propagates invalid fluid state. Thus the new failure path does not report successful advancement after detecting the known numerical failure.

Work counters cover lattice advances, dispatches, submissions, uploads and readback volume, including preparation/migration work since `begin_frame`. Timers are CPU wall measurements. Readback wait can include prior GPU work, no GPU kernel duration is claimed, and independently calculated stage p95 values cannot be added. Full-step timing surrounds `try_tick`; snapshots, Godot conversion and rendering are excluded and documented.

The probe uses default pressure 12, a controlled unshifted warmup/hold, phase-labelled subsequent movement, and per-case completion/error records. Failed cases do not produce a success row for the failing frame, remain identified as incomplete, and cause a nonzero overall exit. The retained short dataset and held Fine/fork reproduction are described without a numerical-root-cause claim. The moving/fixed reference is correctly labelled zero-cell and uncoupled from the robot, with its possible one-frame migration lag disclosed.

## Latest minor corrections (`7727c60`)

- `crates/simulation/examples/lbm_window_probe.rs:245–252` anchors the first warmup entry's time once, when entries are first observed, and derives later times from that anchor. The anchor no longer moves forward when the eight-entry cap is reached before the end of a later frame. The historical committed migration CSV is preserved, with its old one-substep-late tail caveat at `docs/research/2026-10-28-lbm-streaming-window/README.md:9`.
- The fixture uses `half_width: 3` at `crates/simulation/examples/lbm_window_probe.rs:41`, so physical width is 6. README line 39 correctly gives mean speed approximately `18.5 / 6 = 3.09` and 8/16-unit advection times approximately 2.6/5.2 seconds. Those numbers are labelled planning scales, not evidence that all boundary transients have equilibrated.
- README line 13 accurately states the full-step timing exclusions and lack of GPU kernel timing.

## Evidence inspected

Read the approved streaming-window and production-boundary specifications, opening-numerics note, implementation plan, binding constraints, complete gate ledger, and supplied full logical diff. The logical diff was read in bounded chunks; unrelated earlier research history was not re-audited.

The parent supplied fresh passing evidence for `cargo fmt --all --check`, `cargo check --offline --locked --workspace`, `cargo check --offline --locked -p godot_bridge --features lbm-gpu`, and `git diff --check`. The ledger records the required existing `scripts/check.sh`/Godot headless check and focused real-software-GPU numerical, migration, coupling and error regressions. These are inherited validation evidence, not commands rerun by this reviewer. The latest correction separately has example compilation/fmt/diff evidence. No passed suite was repeated.

This review additionally parsed the committed `functional.csv` read-only with Python's csv module. It confirmed 10 cases/272 rows, pressure 12 in every row, software=true throughout, zero region switches during all recorded hold phases, 30 rows in each completed case and 16 in each failed Fine/fork case. The 72-total-cell cases have 8–11 active local cells. Maximum recorded opening-flux residual was about `8e-6`. These checks corroborate bookkeeping only; tiny prescribed boundary residuals do not prove interior stability or physical accuracy.

## Hardware and acceptance limits

At this review, paid hardware resumption remains pending outside this review task. No hardware performance result for the production window, long-horizon acceptance or shipping recommendation is available.

- Default-pressure Fine/fork is unsupported: held instability shows that migration is not required to reproduce the failure. It must remain excluded from acceptance unless separately resolved and measured.
- The default `small_world` spawn geometry lacks supported opening buffers. A successful feature build or a fixture run does not mean the default LBM game can launch.
- Completed Coarse cases establish only short-horizon software functionality. Physical-GPU p95, full time-series stability, buffer/transition choice, strip warmup adequacy and a shipping profile remain Task 7 work.
- The workload has 72 total cells but only 8–11 locally active cells. It cannot be represented as a 72-cell simultaneous local-coupling benchmark.
- Raw density sum is dimensionless and resolution-dependent. Fine/Coarse mass sums cannot be directly compared as physical mass, and boundary flux equality alone cannot establish density/mass stability.
- Future hardware results require separate technical assessment, including individual case status and time-series transients. This code review does not pre-approve those conclusions or revive paused Stokes research.
