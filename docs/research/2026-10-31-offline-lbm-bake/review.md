# Final whole-branch review

Reviewed `feat/offline-lbm-bake`, `fff9f95..583dca7` (HEAD `583dca77e79e7a0e255c48ca7f116085ad63c270`), against the design, implementation plan, progress ledger, global constraints, three task reports/reviews, and complete supplied diff package. Read unchanged solver, stencil, geometry and background code where needed to check the affected interfaces and acceptance contract.

## Verdicts and findings

- **Spec compliance: Compliant**, including the controller-authorized explicit export resampling and revised bounded software schedule.
- **Task quality: Approved.**
- **Critical: none. Important: none. Minor: none.** No practical correctness, integration, provenance or scope problem requiring a change was identified. No additional test or speculative reliability mechanism is requested.

## Cross-layer review

1. **Shared solver and realtime compatibility.** `crates/fluid-gpu/src/lbm/gpu.rs:118` is the single solver implementation. `DomainSpec` replaces profile-dependent dimensions throughout initialization, parameters, buffers, dispatches, sampling, flux diagnostics and LSQ construction. The existing MRT/Guo/LSQ shader and stencil remain shared; no shader or collision implementation was copied. `crates/fluid-gpu/src/lbm/domain.rs:13` checks positive finite dimensions, index/allocation arithmetic and representable bounds; construction checks numerical scaling and requested/adapter buffer and dispatch limits at `gpu.rs:132`. Unsupported requests return errors without fallback. `crates/fluid-gpu/src/lbm/window.rs:12` retains the previous public realtime API with a private core and explicit delegates. Its shift passes the original exact integer block displacement and updates its public spec only after success (`window.rs:37`). The core retains exact overlap-copy semantics and rejects resolution/spacing changes (`gpu.rs:725`). Existing profile constants and world bounds remain unchanged.

2. **Actual bounded mean and units.** `crates/fluid-gpu/src/lbm/bake.rs:16` validates the finite frame/step budget and requires two or more samples. `bake_mean` advances actual shared solver populations, applies the existing mean-density control after every logical frame, and samples completed strides (`bake.rs:74`). Errors from advancement, control, reads and diagnostics propagate. The accumulator rejects nonfinite/invalid fluid states, uses f64 sums, independent odd-half denominators and total-sample averaging, and exports solid zeros. World velocities come from the existing macro readback scale; `BakedFlow.viscosity` and report dt derive from the actual config (`bake.rs:108`). The report preserves actual schedule/count, simulated durations, sampled density/flux and half-mean changes. No convergence state or automatic extension appears.

3. **Coordinates, mask and file contract.** `crates/simulation/src/offline_lbm.rs:8` preserves row order and inverse solver-solid mask and uses `domain.origin + spacing/2`. `crates/simulation/src/map/fluid_asset.rs:8` and `:49` implement the agreed 52-byte BFLOW001 little-endian header, 16-byte port records, f32 velocity pairs and byte mask. Checked sizes, exact payload length, numeric structure and mask validation reject malformed input. Structural decoding is kept separate from normal background semantics. No dependency or alternate asset model was introduced.

4. **Explicit export adjustment and unchanged acceptance.** `offline_lbm.rs:37` resamples actual source velocities with the production bilinear helper, checks the entire target rectangle lies within source interpolation support, derives the target mask from map geometry and zeroes walls/solids. Metadata is preserved. The example explicitly selects the authorized same-h, half-step-shifted, n-1 export (`crates/simulation/examples/offline_lbm_bake.rs:126`). The `background.rs:115` and `:130` changes only expose existing helpers within the crate; original mask/reference/profile/flux checks and tolerances remain intact. There is no analytic substitution or flux normalization. This local correction remains within the approved static-field route.

5. **Real file import.** The example uses genuine GraphNetwork Q=8, the specified 80-unit fixed square and bounded 300/600/10 schedule. `offline_lbm_bake.rs:136` writes and reads the actual export file, compares velocities/mask, installs the loaded field, invokes the normal policy constructor (`:162`), and checks six boundary/interior/wall samples. Import rejection is recorded and propagated before a success message. Default maps, entrypoints, realtime dimensions and gameplay behavior are not changed.

## Artifacts and verification evidence

The supplied diff package exactly matches the actual `git diff --unified=10 fff9f95..583dca7`. A read-only SHA-256 comparison confirmed all 11 manifest entries match current bytes. The committed source base covers Tasks 1/2; four individually hashed final Task 3 sources, including the new example, identify the successful revised source. The missing original uncommitted example hash is explicitly disclosed and is not presented as complete provenance.

Read-only binary inspection confirmed all three files have finite velocities, legal masks and exact BFLOW001 payload sizes:

| Artifact | Origin | Size | h | Bytes | Fluid nodes |
| --- | --- | --- | --- | --- | --- |
| Initial failed raw field | [-15.75,-15.75] | 160² | .5 | 230484 | 1920 |
| Revised raw field | [-15.75,-15.75] | 160² | .5 | 230484 | 1920 |
| Revised wall-aligned export | [-15.5,-15.5] | 159² | .5 | 227613 | 1749 |

For every revised export node, a read-only comparison against the retained raw field confirmed the specified half-step bilinear values with f32 arithmetic (maximum component difference **0**), authoritative straight-tube mask and exact wall/solid zeros. This checks the retained export's derivation; it does not rerun the solver or product validator.

The first report retains the genuine original port-profile rejection after .8+.8 simulated seconds and 10 samples. The successful report retains 12+24 simulated seconds, 60 samples, sampled density .99796957..1.0018934, opening flux means approximately -8/+8, half-mean RMS .0004220915/max .0010745374, exact file velocity/mask readback, six actual samples and normal import `Ok(())`. Raw and resampled revised fields remain independently inspectable.

Inspected the saved commands/results and logs: fmt exit0; simulation default 40 unit + 5 integration passed; feature 53 unit + 5 integration passed with 10 GPU diagnostics ignored; fluid-gpu LBM CPU suite 7 passed with 6 ignored; workspace all-target feature compile exit0. Task 1's prior exact GPU shift diagnostic passed and addresses the changed migration owner. Completed tests and GPU jobs were not rerun. No named concrete gap justified an additional execution.

## Declined-to-judge boundaries

- **Statistical convergence or physical accuracy:** the schedule and two half means describe one bounded output; they cannot establish convergence. No independent physical-reference or refinement study is in scope.
- **Heart/organ geometry, arbitrary curved walls or openings, and absolute pressure response:** the retained successful asset is a fixed straight-pipe fixture. The wall-aligned export demonstrates that fixture; it does not establish general geometry compatibility or relax existing LSQ/port restrictions.
- **Performance, hardware behavior, Godot/UI and long-range coupled gameplay:** the evidence is one local llvmpipe data-pipeline run plus existing checks. Wall duration is descriptive, and no performance, cloud, real-GPU or interaction work was authorized for this review.
- **Every default-ignored GPU diagnostic:** they remain explicitly ignored; the selected shift result and actual fixed-domain bake provide bounded evidence for the changed paths, not an assertion that all GPU cases were rerun.
- **Exact reconstruction of the first failed executable source:** its missing uncommitted example hash is disclosed. That historical failure supports diagnosis and is not used as successful acceptance evidence.

These are documented scope limits, not completion blockers or new work requests. No source/index/HEAD/branch mutation or nested delegation occurred. Only this requested ignored review report was written.
