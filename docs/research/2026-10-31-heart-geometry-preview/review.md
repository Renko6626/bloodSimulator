# Final whole-stage review

Reviewed `76050f6..d67e61f` on `feat/heart-geometry-preview`, based on the completed offline-LBM stage. The assessment below covers this geometry-preview stage, not a new review of the preceding offline-LBM implementation.

## Strengths

- The chamber outline is the sole geometry representation. `crates/simulation/src/map/chamber.rs:12` checks finite vertices and local nondegeneracy, accepts either winding, and closes the last edge implicitly. Its segment distance plus ray-parity sign supports the approved simple concave polygons. Bounds are derived rather than separately editable.
- `crates/simulation/src/map.rs:292` and `map.rs:310` both include chamber scores while preserving graph-edge ownership. `crates/simulation/src/map/geometry.rs:46` includes the outlines in whole-map bounds. Chunk floor and collision generation sample that same production query; there is no preview-only visual geometry or full-map mesh path.
- `crates/simulation/src/map/heart.rs:46` defines four independent closed outlines, and `heart.rs:93` defines a connected ten-edge circulation cycle. The AV passages and outflows use 9/10-unit-wide static tubes. The return connections are outside the heart body; the local topology test at `heart.rs:174` checks right/left isolation and additionally removes AV connectors to detect same-side outflow bypasses. Inspection found no concrete unintended connection through the septum or atrial wall.
- `crates/simulation/examples/heart_preview_probe.rs:15` uses bounded real `MoveInput` on the normal streamed simulation. It asserts arrival from actual robot poses, body clearance, zero quantities/pressure/flow and the 25-chunk peak. The recorded actual ventricular spans are 83.985 and 89.941 units, both exceeding the agreed 80-unit criterion. The minimum sampled center-to-wall score is 2.790, comfortably above the production robot's 0.85 radius plus the existing wall thickness.
- Preview lifecycle changes are small and mode-specific. `crates/simulation/src/runtime/heart_preview.rs:11` zeros pressure, pulse and both cell counts before streamed construction; `runtime.rs:499` preserves preview and pause on reset; `runtime.rs:562` enforces the zero configuration before actual steps. The existing streamed constructor still owns one Rapier world and loads the fixed 5×5 neighborhood, with no eager full-map geometry. Its empty population remains empty under zero flow.
- The UI guards align across layers. `godot/world.gd:39` rejects incompatible argument families before scenario selection, implies streaming/zoom2, and uses the preview factory. `godot/world_hud.gd:192` marks preview before changing controls, sets pressure without emitting a signal, and disables pressure, drag and flow. Speed changes emit zero pressure at `world_hud.gd:149`; the F handler and world flow callback both block flow enablement. The bridge also clamps preview pressure to zero at `crates/godot_bridge/src/lib.rs:144`. Existing non-preview branches retain their prior behavior.
- Scope stayed within the approved route: no new dependencies, cardiac fluid solver/bake, cell inventory/distribution mechanism, dynamic valve, alternate physics world, cloud work or performance experiment. The remaining map-literal edits only add empty chamber collections and preserve existing numerical geometry.

## Issues

### Critical

None.

### Important

None.

### Minor

None. No parked finding or requested follow-up fix.

## Evidence inspected

Read the approved design and implementation plan, global constraints, full packaged implementation diff and corresponding committed source diff, all changed production/probe code, and directly relevant construction, streaming, movement, flow sampling, population, white-cell seeding, snapshot, bridge and Godot signal/reset callsites. Read task reports/reviews and the progress ledger as supporting context, rather than as a substitute for source inspection.

Inspected the raw default/feature test logs, workspace check and project-script log, startup rejection records, probe log, final import/load logs, source-provenance record and the actual saved 1280×800 game screenshot. All eight named source/probe SHA256 values match the provenance file. The screenshot shows a normal zoom2 spawn view, explicit static/no-flow/no-cell labeling, zero quantities and flow, 25 chunks and fully visible retained controls. It supports local rendered startup, not a whole-heart overview or frame-rate claim.

Recorded checks accepted from this implementation:

- `cargo fmt --all --check`: exit 0.
- `cargo test --locked -p simulation`: 45 unit and 5 integration tests passed.
- `cargo test --locked -p simulation --features lbm-gpu`: 58 unit and 5 integration tests passed; 10 existing cases ignored.
- `cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`: exit 0.
- `bash scripts/check.sh`: exit 0, including ordinary world/lab loading and the first Godot import.
- `cargo run --locked -p simulation --example heart_preview_probe`: exit 0; all 19 navigation targets reached, actual spans and clearance recorded, zero-state guards passed.
- Normal/redundant preview startup: exit 0; nine incompatible argument cases: exit 1 with the intended startup error.
- After the final preview-only HUD statistics shortening: affected cached import and preview load exited 0, and the final static screenshot was captured. No Rust change followed the concentrated Rust verification, so a second full Rust run was unnecessary.

This review did not rerun tests, launch Godot/browser/UI, automate input or alter source/index/HEAD/branch. Its only write is this report. A concurrent untracked controller-notes file is outside the committed review range and was left untouched.

## Declined to judge

- Nonzero heart flow, chamber resistance, pressure accuracy and cardiac LBM baking: explicitly deferred beyond the static zero-flow geometry stage.
- Chamber cell distribution/transport, contraction and dynamic or one-way valves: explicitly excluded from this approved stage.
- Complete remote body-return traversal and arbitrary player trajectories: the approved bounded probe covers the specified route through both ventricles and pulmonary return; it does not claim every route.
- Human Godot movement/control feel, wall-edge feel and interactive UI acceptance: the agreed Godot verification boundary is loading and a static screenshot; production Rust movement/reset evidence is assessed separately above.
- RTX/FPS, performance optimization and cloud execution: paused/out of scope, with no performance claim made here.
- Holes, arbitrary polygon editing and self-intersection repair: excluded by the simple authored-outline contract.

These are documented acceptance limits, not undisclosed defects or new work requests.

## Assessment

**Spec: Compliant. Quality: Approved. Ready to merge this geometry stage: Yes.**

The implementation and evidence satisfy the approved explorable four-chamber geometry preview, including actual reachable ventricular spans, shared physical/rendered geometry, fixed streaming bounds and zero-state/UI protections. Approval is limited to that stage and does not authorize push/merge or imply completion of heart flow, cells, physical accuracy, human UI acceptance or performance validation.
