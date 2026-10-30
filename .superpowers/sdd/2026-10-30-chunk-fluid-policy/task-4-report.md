# Task 4 runtime integration report

Status: complete. Implementation stays in the approved Rust/WGSL, one-window, one-Rapier-world route.

## Changes

- Added fallible streamed policy-world constructors. CPU preparation validates background assets/configuration and (when requested) LBM options and peak-drive candidates before allocating a GPU window or constructing bodies. A forbidden spawn allocates no GPU window while retaining its LBM request.
- Added `PolicyRuntime`, independent of `Simulation.lbm`. Reset preserves the policy map, existing local population configuration, white-cell enablement and pause state. Profile reset rebuilds candidates from the saved request even without an active window.
- Added explicit-origin window/coupling APIs while preserving position-based wrappers. Policy frames select only prepared centers at frame start, create/migrate/release one window, then hold its origin through all mechanical substeps. Errors from GPU construction/preparation/advancement propagate directly.
- Added common `frame_activation` (legacy default 1). Existing local sampling now supplies a*w to reaction exchange, background drag, robot/white-cell velocity composition, arrows and world samples. Existing immunity code already uses this common sampling path and needed no source change.
- Policy backgrounds sample `PreparedBackground` with `flow.summary.pressure_drop`; legacy maps retain graph sampling. Policy `sample_world` remains available with zero local weight and a valid background when no window exists. Legacy no-window calls still return None.
- Config validation precedes environment/body/lifetime/time/GPU mutation. Saved LBM options are validated even when no window is active.
- Exposed `PolicyFluidStatus` without changing Godot snapshot fields. Its center is the current robot chunk; activation/window are selected at frame start. On a crossing frame, use existing LBM snapshot bounds to identify the selected window.
- Old custom-map constructors explicitly direct policy maps to the new fallible entry points. Aligned the background maximum-pressure comparator to require strictly positive values and extended its existing rejection test.

## Verification

All commands used `CARGO_TARGET_DIR=/data/sunyunbo/www/bloodSimulator/target`.

- `cargo test --locked -p simulation runtime::policy`: 2 passed (default build).
- `cargo test --locked -p simulation runtime::policy --features lbm-gpu`: 3 passed. These are CPU tests; the LBM-request test uses a forbidden spawn and never requests an adapter. Covers no mutation on invalid drive, valid advancement/reset, approved-center-only selection, inactive request/profile retention, and config/options validation with no window.
- `cargo test --locked -p simulation background::tests`: 5 passed, including the added zero-maximum rejection.
- `cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu`: passed.
- `git diff --check`: passed.

Self-review checked construction ordering, the request/active-window separation, frame-locked origin/activation, common force/sample weights, error propagation and legacy-path selection. No additional architecture decisions or mechanisms were introduced.

Per controller's scoped-validation ruling, the full default suite and `scripts/check.sh` are deferred to Task 5 consolidated validation. Actual GPU entry/exit/re-entry and crossing-frame replay remain Task 5 evidence; this task does not claim that runtime GPU validation. No cloud instances, profiling, solver changes or UI automation were used.
