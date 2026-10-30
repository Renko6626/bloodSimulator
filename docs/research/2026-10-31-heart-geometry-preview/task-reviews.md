# 逐任务审阅


---

# Task 1 review

## Verdicts

- **Spec: compliant.** Task 1's polygon, shared geometry-query, bounds, legacy-map, and focused-test requirements are met.
- **Quality: Approved.** No blocking or nonblocking findings within this task's scope.

## Evidence and reasoning

Reviewed the supplied `76050f6..e4c49df` diff, task brief, global constraints, implementation report, and the relevant map sources. Source, index, HEAD, and branch were not modified. Only this review record was added.

- `crates/simulation/src/map/chamber.rs:12` checks the finite, locally nondegenerate outline, accepts either winding, and removes an optional repeated closing vertex. Iterating each vertex with its cyclic successor includes the implicit final edge. The minimum segment distance and ray-parity sign support simple concave polygons. Self-intersection repair and holes remain outside the approved scope.
- `crates/simulation/src/map/chamber.rs:34` derives bounds from the sole boundary representation, so it introduces no independently editable or stale geometry cache.
- `crates/simulation/src/map.rs:291` takes the minimum of vessel and chamber scores. `crates/simulation/src/map.rs:310` includes the same chamber scores in the combined query while selecting ownership exclusively from the existing graph edges. The existing nonempty-graph requirement is retained.
- `crates/simulation/src/map/geometry.rs:46` includes chamber vertices in both axes of whole-map bounds. Existing chunk and whole-map sampling use the shared `signed_distance` implementation, so floor and collision walls include chambers without a parallel meshing path.
- The remaining production/example/test literal changes in the supplied package only add empty chamber collections. Existing map numerical values are unchanged, and the empty iterator preserves legacy geometry-query results. Archived experiment sources are deliberately outside this change.
- The three added tests cover winding and concavity, graph ownership and combined-query agreement, and a chamber outside tube bounds with floor/walls crossing a 16-unit seam. They exercise production geometry rather than duplicate the SDF algorithm.

## Verification boundary

The implementation report records `cargo test --locked -p simulation map::` passing all 8 selected unit tests, with 5 integration tests filtered out, plus formatting and diff checks. As instructed, this review did not rerun passed suites or start GPU/UI tests. Feature/all-target compilation remains the explicitly scheduled Task 3 check; this approval does not claim it has already passed. Heart layout, robot clearance, preview runtime controls, and rendering acceptance belong to subsequent tasks.

## Findings

None. No severity/file:line issue requires a change before Task 2.


---

# Task 2 review

Spec: compliant for the Task 2 scope. Quality: Approved. No Important or Critical findings; no changes required before Task 3.

Reviewed the task brief, global constraints, report, packaged `e4c49df..5e1c188` diff, and the named production call sites for streamed construction, movement, snapshot flow/cell reporting, white-cell seeding and reset. Source/index/HEAD were not changed. No tests, probe, GPU or UI suites were repeated, and no agents were spawned.

## Compliance

- `crates/simulation/src/map/heart.rs:46` defines four named closed cubic-sampled chamber outlines. The cubic formula is standard and does not modify the legacy vessel helper. The ventricle extensions preserve the agreed reference's basic shape while allowing actual traversable depth. The exterior return routes sit beyond the heart body rather than crossing the septum.
- `crates/simulation/src/map/heart.rs:93` defines ten sequential graph nodes/edges forming one external circulation cycle. The four cardiac interfaces are distinct; the AV necks and outflows are static 9/10-unit-wide tubes. Intracardiac guide centerlines stay on their corresponding side. The local ROI flood checks exclude the normal external circulation and additionally remove AV tubes to check for same-side outflow bypasses. No concrete topology shortcut was found.
- `crates/simulation/src/runtime/heart_preview.rs:11` reuses streamed construction and its existing single Rapier world, with pressure, pulse and both cell counts zeroed before construction. The ordinary constructor initializes the private preview flag false. `runtime.rs:560` enforces zero config before each actual step; `runtime.rs:495` handles preview reset ahead of policy/LBM/ordinary streaming reset and preserves pause. Mutable nonzero pressure/counts therefore do not reach a preview physics step. Existing streamed construction's local population is empty, so the subsequent Full config enforcement does not introduce cells.
- `crates/simulation/examples/heart_preview_probe.rs:15` drives the normal instance with bounded real `MoveInput`, checks actual arrival and reads actual simulated poses. Its normalization and neck/turn waypoints are confined to the validation controller/declared route; no production movement, collision or geometry guard was relaxed. The report records actual right/left spans of 83.985/89.941 units, both above 80, rather than inferring them from outline bounds.
- The reported minimum center-to-wall distance is 2.790, against the real ball radius 0.85 (`crates/simulation/src/robot.rs:13`). The probe asserts at least 1.0 at every sampled post-tick pose, alongside zero active cells, zero white bodies, zero branch flows/summary pressure and a 25-chunk peak. This supports the declared route's body clearance and traversal, not arbitrary input paths or anatomical/fluid accuracy.

## Quality and verification boundaries

The implementation is small and stays within the approved zero-flow/zero-cell preview: no dependencies, scene registry, extra physics world, inventory, fluid baking, cloud or performance work. Geometry construction, preview lifecycle and replay validation are kept in their task-owned modules. No practical completion blocker was identified.

Accepted existing Task 2 evidence: the two focused heart tests and final real probe passed, plus reported formatting and diff checks. This review performed source inspection only. Godot/HUD startup and parameter-write protection, screenshots, and full default/feature checks remain Task 3. The far body-return loop, nonzero flow, arbitrary player trajectories and physical accuracy remain unverified and are not claimed by this approval.


---

# Task 3 review

Reviewed range: `5e1c188..d67e61f` in `/data/sunyunbo/.codex/worktrees/4aad/bloodSimulator`.

## Verdict

- Spec: **Compliant** within the approved static, zero-flow/zero-cell preview scope.
- Quality: **Approved**.
- Findings: **None**. No Critical, Important, or Minor issue requiring a change was identified in this task's implementation or delivery evidence.

## Scope and method

Read the task brief, global constraints, review instructions, implementation report and packaged diff. Reviewed the changed bridge and Godot code with its directly relevant signal, factory, reset, step and streaming callsites. Inspected saved command results, startup rejection logs, probe output, source provenance, research documentation, STATUS/plan changes and the final actual game screenshot.

This was a static review. No suites, GPU checks or Godot processes were rerun, and no keys, mouse clicks, forms or UI workflows were exercised. The only additional concrete evidence check was `sha256sum` for the eight production/probe files named by `source-provenance.txt`; every digest matched. Source, index, HEAD and branch were left untouched; this report is the sole review output.

## Requirement assessment

1. **Explicit entry and incompatible arguments:** `godot/world.gd:39` detects `--heart-preview`. The rejection at `world.gd:40` covers local LBM/Stokes, validation, profile, every old benchmark parameter family and screenshot family before the startup scenario-factory calls. Preview implies streaming and zoom2 at `world.gd:45`; redundant `--stream-world` and `--local-cells` reach the preview factory, whose existing zero configuration overrides population mode/count. Non-preview branches retain their prior factory routing at `world.gd:96`.
2. **Bridge zero state:** `crates/godot_bridge/src/lib.rs:70` forces streaming, constructs the approved preview, and clears validation/error state. The parameter entry at `lib.rs:144` forces preview pressure to zero while preserving existing speed/drag clamps. The directly relevant existing simulation contract zeros pressure, pulse and populations in `runtime/heart_preview.rs:3`; reset selects the preview before ordinary streaming/policy paths at `runtime.rs:499`, and stepping enforces its configuration at `runtime.rs:562`.
3. **HUD and signal order:** The child HUD builds its controls before the world's `_ready` calls `set_heart_preview` (`world.gd:95`), and the world connects its signals afterwards (`world.gd:112`). `world_hud.gd:192` marks the preview before setting pressure without a signal, disables pressure/drag, and clears/disables the flow toggle. The actual speed-slider callback emits zero pressure at `world_hud.gd:149`; both the HUD F path (`world_hud.gd:139`) and world flow handler (`world.gd:218`) guard against enabling flow. Pause, step, reset, movement, speed and collision handlers remain available.
4. **Ordinary HUD behavior:** The new concise statistics branch is conditional on `heart_preview` at `world_hud.gd:131`. Ordinary statistics, labels, slider signal behavior and flow toggling retain their existing paths. Turning former local label references into members does not alter ordinary display behavior.
5. **Visible output:** Inspected `docs/research/2026-10-31-heart-geometry-preview/preview-startup.png`. It is a 1280×800 normal spawn game view with the robot, tricuspid region, explicit static/no-flow/no-cell description, zero populations/pressure/flow/speed, 25 chunks, disabled pressure/drag/flow controls and a fully visible collision toggle. It is appropriately documented as a local zoom2 view, not a whole-heart overview or performance/physical validation.
6. **Delivery evidence:** `logs/results.txt` records all required checks exiting zero. Raw logs support 45 unit plus 5 integration tests by default and 58 unit plus 5 integration tests with the feature, with 10 existing ignored cases. Workspace feature compilation and project script logs show successful build/import/ordinary-world/lab loads. `startup-results.txt` and individual logs support normal/redundant preview launches exiting zero and nine incompatible cases exiting one for the intended startup error. The probe log records all 19 targets, actual ventricular spans 83.985/89.941, minimum clearance 2.790 and the zero-state guards.
7. **Final GDScript-only revision:** The preview-only stats shortening responds to an observed screenshot overflow. The saved affected import/startup logs and final screenshot correspond to that revision, and all source-provenance hashes match the reviewed files. Limiting the follow-up checks to Godot loading and the static capture was appropriate; no Rust behavior changed and no full Rust rerun was needed.
8. **Scope discipline and documentation:** Research notes and STATUS state the static preview boundary, pending human interaction/feel checks and absent heart-flow/cell/valve/physical/performance acceptance. The plan leaves whole-branch independent review pending. No added dependency, inventory mechanism, cardiac baking/flow, cloud/performance work, push or merge is part of this task. The capture record identifies its owned PID/XID and static capture method; there is no evidence of automated UI interaction.

## Remaining acceptance limits

Human Godot movement, pause/reset, sliders and wall-edge feel were not exercised; that is explicitly allowed by the task's loading/static-capture verification boundary. The underlying simulation traversal/reset evidence does not claim human UI acceptance. Whole-branch review remains a separate parent-agent step. Neither limit is a Task 3 completion blocker.
