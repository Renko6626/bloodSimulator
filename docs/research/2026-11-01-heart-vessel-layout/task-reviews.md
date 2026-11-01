# 逐任务审阅


---

# Task 1 review

Reviewed commit: `7dad8730ef4a532173810d931897957cb5713adf`, packaged range `5e6bbec..7dad873`.

**Spec verdict: PASS for Task 1.**

**Quality verdict: PASS.** No Critical, Important, or Minor findings requiring changes were identified in the reviewed implementation. This approves proceeding to Task 2; it does not accept the whole feature before Task 2 evidence exists.

## Scope and evidence

Read the task brief, global constraints, review instructions, approved specification, implementation report, packaged diff, and complete production `heart.rs`/`heart/vascular.rs`. Inspected the direct `MapDefinition` SDF/length APIs, production `GraphNetwork`, preview runtime initialization/reset/zero enforcement, default viscosity, and existing bounded-input probe. Working HEAD matched the packaged implementation. No tests, Godot, GPU work, or input replay were run during this review. The only written file is this report; source, index, and branch were not changed.

The implementation report records four focused tests passing, fmt passing, and diff checks passing on this code. Those are builder-supplied results, not a fresh reviewer execution.

## Specification checks

- **Actual geometry and serial topology:** `vascular.rs:79` constructs two independent mirrored trees and joins each supply/return leaf pair with four actual microtube polylines. Each module contains 2 D24, 8 D12, 32 D6, and 64 D3 edges, totaling106. `vascular.rs:139` replaces the original short/rectangular returns with the two modules in series. The original ten narrow/internal edges plus two return connectors produce224 edges and98 nodes. There is no bypass graph edge, speed assignment, or resistance override.
- **Independent microtubes and physical separation:** the64 seam centerlines occupy distinct y coordinates at16-unit intervals, with D3 lumen width. They share only their legitimate fork endpoints. Hairpin lanes are separated by18/10/4 units for diameters12/6/3 respectively, so their nonadjacent parallel legs remain distinct even within one edge. The forward fan pieces are straight rays diverging from a shared node; reflected return pieces converge to the corresponding node. Inspection found no second crossing hidden by the shared-junction exemption in `vascular.rs:310`. The capsule-pair test checks the remaining cross-edge pieces, and actual SDF assertions check all64 seam lumens and63 separators per module (`vascular.rs:344`).
- **Junction-test limits were considered:** the exemption skips complete segment pairs when each has an endpoint near the shared node; it does not literally clip segments to a small disk. Also, the pair loop starts at edge12 and does not inspect same-edge pairs. These are limits of that assertion, not evidence of a present defect: the current straight-ray junctions, authored hairpins, and four narrow-to-trunk connections were also inspected directly. The report correctly qualifies the measured5-unit minimum as a gap between different non-junction external edges; it is not the minimum within a hairpin.
- **Real branch symmetry and production rates:** `vascular.rs:16` compensates actual centerline length, not solver values. For each fan, its length is `longest_diagonal + tail + 2*base + gap + 4*lane`, independent of child index. The resulting D12 and D6 lengths are approximately756.77155 and283.37764; a microtube has two108-unit half-fans plus the32-unit seam, totaling248. The production solver test (`vascular.rs:194`) normalizes by its actual pump rate, checks every graded edge against48/12/3/.75 with absolute tolerance1e-4, and checks every node's signed conservation residual. These tolerances are small relative to the targets and do not conceal a material branch imbalance. Series trunks may have different lengths without changing parallel symmetry.
- **Ports and domain:** narrow extensions retain axes x38/78/112/150 and D10/D9/D10/D10 at y=-32 (`heart.rs:114`, `heart.rs:128`, `vascular.rs:139`). All D24 transitions begin at y=-64, whose endcaps reach only y=-52, outside the heart domain's top boundary. The upper module, lower module, and x±700 body corridors remain outside `[-32,224]^2`; lower-module geometry lies beyond y272. The reported complete capsule bounds `[-712,-1305.5]..[712,1413.5]` are consistent with these paths. No original outer rectangle or short lung bridge remains.
- **Chambers and APIs:** chamber curves, spawn, internal paths, and AV-edge indices1/6 are preserved in `heart.rs`. Existing septum/AV flood assertions remain intact (`heart.rs:175`). Factory and route signatures remain unchanged. Only the two owned files changed, with a private heart-specific helper and no public protocol, dependency, or runtime-flow change.
- **Navigation handoff:** `heart.rs:23` retains the entry/deep names and orders right ventricle, full lung microtube, left ventricle, full body microtube, and right atrium return. `vascular.rs:152` uses the same authored branch/trunk geometry, includes every hairpin corner, and subdivides long legs to at most96 units. The existing probe's2400-tick budget is per waypoint, so total route length alone does not violate that budget. Repeated vessel samples are excluded from map landmarks. These observations establish the intended route and compatible waypoint spacing, not successful physical traversal.
- **Zero behavior and scope:** the unchanged preview runtime still zeros pressure, pulse, red/white counts, and uses the streamed map constructor. This implementation adds neither a baked field nor running preview blood flow, and changes no default/Local/LBM entry path.

## Remaining acceptance boundaries

The numeric reference605.5543 and unit flow0.07926622 explicitly use viscosity1 in `vascular.rs:198`. Runtime `Config::default()` uses viscosity0.05. The viscosity1 result is valid as labeled and is not a runtime-default pressure reference; Task 2 must calculate and label the actual default-viscosity reference with the production solver.

Task 1 has not supplied actual bounded MoveInput traversal or measured collision clearance. SDF-safe waypoint positions do not prove the robot can negotiate every turn with streamed collision geometry. Task 2 still owns that replay, the real ventricular spans, per-frame zero-state checks, peak25 loaded chunks, and minimum actual center clearance1.0 with the existing0.85 robot radius. It also owns whole-project checks, headless loading, source-backed overview/detail artifacts, and actual solver CSV output. No such future evidence is treated as already delivered here.

Future field baking, physiological pressure/velocity accuracy, LBM acceptance, performance tuning, and cloud experiments are outside this gate and do not create additional Task 1 requirements.


---

# Task 1 fix round 1 review

Reviewed range: `7dad873..5ea9431`; working HEAD `5ea9431b2a0a40b127bde744415e1a258d73631d`.

**Issue status: ADDRESSED. Spec verdict: PASS for this fix. Quality verdict: PASS.** No Critical, Important, or Minor findings were identified in the fix diff. Final dynamic traversal acceptance remains with Task 2.

The original route jumped from the body return endpoint `(38,-4)` directly to `(45,45)`, while the existing D10 edge0 runs through `(38,32)`. `crates/simulation/src/map/heart.rs:47` now inserts that real centerline corner before the unchanged final atrial target. This directly fixes the diagnosed wall-cutting route segment and preserves the complete body circulation path, existing waypoint names, map geometry, robot settings, and clearance guards. The added name follows the existing recognition-point convention and changes no public signature.

`crates/simulation/src/map/heart/vascular.rs:364` adds a regression check against production `MapDefinition::signed_distance` along consecutive route segments, including spawn-to-first, with samples no more than1 unit apart and sampled center clearance at least1.4. The assertion checks the missing behavior that endpoint-only assertions missed. It does not reproduce the map's distance algorithm, use mocks, or relax existing expectations. Zero-length consecutive points are handled by the existing `steps.max(1)` denominator without creating invalid samples.

Read the packaged diff, changed production source, and appended fix report. The report records the new check failing before the route correction on `right_atrium_return`, then all four focused tests passing after correction, plus fmt/diff checks passing. These are builder-supplied results; this review did not rerun tests, replay, Godot, or GPU work. Only this report was written.

The original real replay reached all210 waypoints but failed minimum actual clearance at0.85198593. That remains a valid discovered defect in the earlier route; the prior Task 1 review did not establish dynamic traversal. The corrected211-waypoint route now follows the missing bend, but sampled planned geometry is not a substitute for a successful production collision/input replay. Task 2 must still report the corrected replay's unchanged clearance guard and other actual traversal checks before final acceptance. No whole-task rereview or additional scope was introduced here.


---

# Task 2 review

Reviewed package: `7dad873..1274b7c`; Task 2 source/delivery commit `1274b7c`. The intervening Task 1 route fix `5ea9431` has its own scoped PASS in `task-1-fix-1-review.md`; this review checked its relationship to the corrected replay without reopening unrelated Task 1 work.

**Spec verdict: PASS for Task 2. Quality verdict: PASS.** No Critical, Important, or Minor findings requiring changes. The parent's final whole-branch review and execution decision remain the separate unchecked plan item.

## Spec and evidence

- **Actual traversal:** `crates/simulation/examples/heart_preview_probe.rs:94` constructs the production heart preview and only overrides the pre-existing diagnostic speed8. Lines112–149 use the shared route, real `Simulation::tick(MoveInput)` and post-tick snapshots/SDF. Each advanced frame checks center clearance≥1.0, no active cells or white bodies, zero branch/total flow and pressure drop, and records the actual loaded-chunk peak. Lines163–187 measure actual ventricular spans and retain both final clearance and peak25 assertions. The production callsites in `runtime.rs`, `runtime/heart_preview.rs`, `runtime/streaming.rs` and `robot.rs` confirm normal physics/substeps, zero-preview configuration, local streaming, and the unchanged radius0.85; the probe does not inject poses or replace simulation advancement.
- **Corrected raw result:** `docs/research/2026-11-01-heart-vessel-layout/actual-probe.log:6` onward contains211 waypoint records, including the entire selected pulmonary microtube around y−1304 and the complete body microtube around y396, the left heart transition and final atrial return. Lines219–220 record87771ticks, sampled distance11684.384, minimum center clearance1.273 at `[-15.7392845,-1303.7727]`, and peak25. Right/left spans are83.985/89.943. The final success line is printed only after the unchanged final assertions; successful completion also establishes the per-frame guards for this run.
- **Failure retained and explained:** `actual-probe-first-failure.log` has210 records,87761ticks and the final clearance panic0.85198593 at `[42.148014,24.957287]`. The earlier snapshot retains the final-only clearance assertion, so reaching every waypoint is correctly not presented as acceptance. The production edge0 centerline contains `(38,32)`; `heart.rs:47` now includes that same missing corner before returning to `(45,45)`. The corrected log explicitly records both approach and return. No route was removed, geometry widened, robot shrunk, or threshold relaxed. The report correctly does not claim the first run's final peak assertion executed.
- **Actual solver export:** probe lines13–36 instantiate the real `GraphNetwork` with `Config::default().viscosity`0.05, solve unit drive, normalize by the actual pump rate, solve again and export signed rates and node pressures. There is no per-edge override. Read the production graph implementation and default configuration. The raw CSV has224 distinct edges and98 nodes; its D24/D12/D6/D3 counts are4/16/64/128, rates48/12/3/approximately0.75 and H1 means2/1/0.5/approximately0.25. Its largest D3 rate deviation is6e−8 and serialized node residual is0. The diagnostic log records unit rate1.5853244 and reference drive30.277714. The delivery distinguishes this default-viscosity value from Task 1's viscosity1 result and from physiological pressure/runtime flow.
- **Source-backed figures:** probe lines41–89 draw actual chamber boundaries and centerlines, using real diameters and round capsules. Read all three SVGs as XML and viewed the three existing PNGs statically. Each SVG contains the same224 polylines and4 chamber polygons; the geometry hashes match across views. All224 widths agree with the CSV; polyline lengths agree within decimal/f32 serialization precision, and radius-inclusive bounds are `[-712,-1305.5]..[712,1413.5]`. Overview, heart ports and microtube detail are legible diagnostic artifacts. The dashed heart-domain box is explicitly a proposed bake boundary, not a flow depiction. Visible titles identify zero runtime flow; the README identifies them as diagnostics rather than game screenshots.
- **Required checks:** inspected raw `check.log`, feature-test logs, both all-target compile outcomes, corrected-route compile/build/load logs, diagnostic output, and `command-exits.log`. They support default47unit+5integration, feature60unit+5integration with10ignored, ordinary world/lab loads, editor import and heart-preview load. E0689 belongs to the diagnostic-title numeric inference and is retained with the explicit f32 fix and successful affected compile. After the route/fail-fast change, affected fmt/all-target feature compile/bridge build/heart load and the full corrected replay are recorded successful. The unrelated completed suites were not repeated. Target paths in logs use the authorized shared target directory.
- **Ownership and delivery:** commit `1274b7c` changes the existing diagnostic example and owned documentation/artifacts only. The map route/test edits belong to the separately reviewed Task 1 commit. STATUS, README, provenance and plan state the result, failed attempt and remaining parent-review step. There are no GDScript/HUD/dependency/production API changes or push/merge actions in the reviewed delivery.

## Quality and provenance

The diagnostic addition stays local to the existing example. CSV export uses actual solver outputs; drawing does not feed data back into simulation. The same-threshold fail-fast guard adds the failing segment, pose and target while preserving the final guard, making the observed route failure easier to locate without adding a new runtime mechanism. The optional diagnostics-only path is appropriate for this offline artifact workflow.

Read both exact probe snapshots and their diff. Their movement, physics and zero-state logic match; subsequent changes are drawing/title/diagnostic selection plus the same-threshold fail-fast guard. SHA-256 checks match provenance for the first probe (`e62041d8…c1329b`), corrected/final probe (`7e66ebc0…f8ccc8c`), both historical/current heart and vascular sources, and current graph/config sources. The final probe snapshot exactly matches the final example. The first failure and corrected success remain distinguishable rather than being overwritten or combined.

This was a read-only source/artifact review, apart from this report. No checks, replay, Godot, UI interaction, GPU work or cloud experiments were rerun; no source/index/branch/commit edits or subagents were used. Review commands read source/logs, hashes and existing CSV/SVG/PNG evidence only.

Acceptance covers the specified geometry/reference graph rates and this production zero-state traversal. Manual feel, arbitrary routes, baked fields, physical chamber dynamics/pressure, cells and GPU performance are intentionally outside this stage. Their absence is documented and is not a Task 2 acceptance gap.
