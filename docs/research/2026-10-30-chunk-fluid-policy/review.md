# Whole-branch final review

Reviewer: Senior Rust simulation/fluid reviewer, sol high. Reviewed `918c641..5ca26fc` in `/data/sunyunbo/.codex/worktrees/4aad/bloodSimulator` against the approved design and completed implementation plan. This is an independent cross-task review; no additional reviewer agents were used.

**Verdict: Approved for the scoped local implementation delivery. No Critical or Important findings. One nonblocking Minor documentation/provenance clarification remains.** This verdict does not authorize push, merge, deployment, additional optimization or experiments. The complete `scripts/check.sh` run remains failed at cold Godot import; its separately successful cached import and scene checks are reported accurately.

## Strengths and plan alignment

- The source policy is a persistent map property with separate background and coverage permission (`crates/simulation/src/map/fluid_policy.rs:6`, `:30`). Unspecified chunks are analytic and forbidden. Candidate centers are derived rather than independently editable; no per-chunk solver, Godot policy copy, dependency or storage layer was introduced.
- The new path uses the existing shared chunk dimensions and scene streaming. Candidate construction requires all nine analytic permissions before geometry work (`crates/simulation/src/fluid/lbm_policy.rs:58`), uses actual profile rasterization/opening reconstruction (`:79`), conservatively rejects pump/window contact and crossings (`:176`), and checks both initial and prescribed full opening speed against the peak-drive budget (`:81`, `:95`). Temporary raster data is released after each check. The 16-unit grid, 3×3 LBM window and 5×5 scene are preserved.
- Planar analytic sampling has the required unit-depth normalization without the legacy extra wall fade (`crates/simulation/src/circulation/graph.rs:173`, `:236`, `:254`). The circular sampler retains its arithmetic and fade. Precomputed fields share world-coordinate samples across chunks, use a single instantaneous/reference-pressure multiplier and reject mismatched geometry revision, viscosity, reference flux, scale range, masks, support and source boundaries (`crates/simulation/src/background.rs:96`, `:147`, `:200`, `:316`). There is no new heart-field generator or implied arbitrary-cavity solver.
- Prepared drive validation precedes environment, lifetimes, population, AI, mechanics and GPU work (`crates/simulation/src/runtime.rs:552`). It rejects changed viscosity/dt/substeps and invalid base pressure. Nonfinite instantaneous pressure is rejected before environment update (`:580`). The maximum preparation drive includes the existing 15% pulse. Turning LBM off does not bypass background-asset validation.
- User request/options and the derived policy survive independently of the active `Option<LbmCoupling>` (`crates/simulation/src/runtime/policy.rs:22`). Preparation happens before GPU allocation and body construction (`:95`). Frame selection creates, migrates or releases the sole window (`:206`); substeps use its selected origin (`crates/simulation/src/runtime.rs:613`), and successful frames finish density control/readback before the next selection (`:706`). Construction/prepare/advance failures propagate; geometry rejection is a distinct prepared background-only branch.
- Activation uses the external boundary of the valid-center union, including holes, while omitting shared internal edges (`crates/simulation/src/fluid/lbm_policy.rs:128`, `:156`). `LbmCoupling::local` applies frame activation to the existing window weight (`crates/simulation/src/fluid/lbm_coupling.rs:171`). Background cell drag, marker body impulse and reverse fluid impulse, robot/white-cell velocity, arrows and public samples consume this common sampling path (`crates/simulation/src/runtime.rs:649`, `crates/simulation/src/fluid/lbm_coupling.rs:273`, `crates/simulation/src/runtime/immunity.rs:313`, `crates/simulation/src/runtime.rs:849`). No second body or fluid advancement was found.
- Reset and profile reset consult the saved request even without a window and rebuild the prepared set (`crates/simulation/src/runtime.rs:496`, `:522`; `crates/simulation/src/runtime/policy.rs:188`). Inactive policy worlds retain background diagnostics/samples. Legacy constructors explicitly reject policy maps rather than silently ignoring them; legacy maps and probe default branches remain on their original route.
- The explicit probe checks actual frame-start selection and reconstructs the active center from snapshot origin, so crossing-frame post-step actor coordinates are not mistaken for the selected window (`crates/simulation/examples/lbm_gameplay_probe/policy.rs:94`, `:100`). The area-guard correction uses per-type radius while retaining prior thresholds and red-only arithmetic (`crates/simulation/src/lbm_validation.rs:194`).

## Findings

### Critical

None.

### Important

None established in the reviewed scope.

### Minor — aggregate source-diff hash scope is incomplete

`docs/research/2026-10-30-chunk-fluid-policy/software/manifest.json:4` records the digest of the three previously tracked Task 5 source files, omitting the new `crates/simulation/examples/lbm_gameplay_probe/policy.rs` from that aggregate diff. Recomputing the four-file committed diff produces `3425cc943b5f05bb46d46527cbd78aa02a9decdabf658862a1cc52320296be32`; excluding the new file produces the recorded `e4497fa39e5eb034c1d8a66e53ba239e7fe8220f6d2bd1cb6bab146cf8771f9f`.

Impact: the aggregate field's implied scope is broader than the bytes it identifies. This is a documentation clarification, not loss of source provenance: the new probe is committed, and its individual source hash is present and correct, as are all other recorded hashes. Optional cleanup is to label the aggregate's tracked-file scope or regenerate it with the complete source list and record the command. This does not block the functional delivery.

## Evidence reviewed

Reviewed the whole source change set, its callers and shared sampling/force consumers, the approved spec/plan, all five task reviews/reports and the progress rulings. Relevant unchanged code was read for graph scaling, geometry authority, white-cell insertion/seeding, scene dimensions and frame readback guards.

No passed suites were rerun. Existing logs support default 38 unit +5 integration tests, feature 49 unit +5 integration tests with 10 ignored, all-target feature compilation, and the separately invoked real inactive-reset test. `scripts/check.sh` completed Rust checks/tests/build but exited 134 at the first Godot editor import; cached import, world and lab commands subsequently exited 0 individually. No claim of a passing complete script or a proven cold-import cause is made.

Independent read-only Python/hash/CSV inspection verified all 10 artifact and four source hashes. The CSV has 600 frames and actual activation events at 87 (create), 375 (release), 447 (recreate), zero frame-start selection mismatches, zero inactive nonzero activation/marker/local weight, and maximum quantity error `1.4210854715202004e-14`. This corroborates the functional report without replaying it or treating software timings as hardware evidence.

Only this requested review report was written. No source, index, HEAD, branch, cloud, deployment or UI state was changed.

## Declined to judge

- **Heart geometry, a real organ asset and its field-generation algorithm:** expressly excluded and awaiting separate design. The straight-pipe asset fixture verifies the present interface, not a heart model.
- **Exact tracing of arbitrary grazing/sliver source boundaries:** preparation deliberately uses discrete asset masks and resolution-based boundary checks, as ruled and disclosed in Task 2. No stronger geometry guarantee is claimed; requiring an exact tracing subsystem would enlarge scope.
- **Strict mass/momentum conservation, absolute pressure fidelity, arbitrary sloped openings or long-run mixed stability:** neither spatial mixing nor mean-density control claims these properties. Candidate peak checks are an input budget, not a coupled-stability proof; the current short functional evidence cannot settle them.
- **RTX FPS, profiling, performance optimization or expanded windows:** no hardware experiment was authorized for this phase; llvmpipe evidence establishes lifecycle functionality only. Prior performance pause and shared dimensions remain in force.
- **Cold Godot import root cause:** the abort is real and accurately retained, with historical similar symptoms and successful cached/scene checks. The available evidence does not uniquely attribute it to this branch. A new environment investigation or workaround was not made a prerequisite without a concrete causal finding.
- **GPU retry/recovery, window history preservation after exit, arbitrary discontinuous repositioning and general hot editing:** excluded from the approved lifecycle and unnecessary for its normal movement/reset path. No recovery framework or additional persistent state is warranted by this review.
- **Full gameplay feel, combat load, arbitrary whole-map travel, and UI interaction acceptance:** the delivered path is an explicit Rust infrastructure/probe entry. No automatic UI exercise was performed, consistent with the user's validation limits.

The approved tasks are complete within their stated delivery boundary. The documented Godot validation limitation and optional Minor should remain visible in the final handoff; neither supplies evidence for widening the implementation task.
