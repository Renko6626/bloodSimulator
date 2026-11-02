# Final whole-branch review

Range: `fed4251..4647d6d`, branch `feat/map-region-render`, worktree `/data/sunyunbo/.codex/worktrees/4aad/bloodSimulator`.

**Spec verdict: PASS. Quality verdict: PASS. Ready for documentation closure: yes.** No Critical, Important, or actionable Minor findings. This verdict does not authorize pushing or merging.

## Review passes and strengths

Reviewed the approved design and implementation plan, task brief, global constraints, task report/review, progress ledger, whole-range change inventory and source diff, actual changed source, source-backed map factories, documentation, and exported artifacts. Reviewed implementation and compatibility first, then verification and delivery evidence.

- The new CLI stays within the approved example-only architecture. `crates/simulation/examples/map_render.rs:231` directly selects the existing heart/small-world `MapDefinition` factories and writes SVG. It constructs no simulation, graph, physics world, engine, fluid backend, or traversal. The existing Vector value used for chamber containment at `:212` does not instantiate a Rapier world. No Cargo/dependency, production map, or public interface changes are present.
- Parsing at `map_render.rs:36` consumes negative coordinate values correctly, requires the rectangle and output path, rejects non-finite numbers and non-positive/unrepresentable dimensions, limits map choices to the two approved factories, and reports errors through a nonzero exit (`:246`). Grid density has a simple rejection bound (`:95`), and grid coordinates remain aligned to the world origin (`:173`).
- The shared writer at `crates/simulation/examples/support/map_svg.rs:13` serializes actual chamber boundaries and complete centerlines, with actual diameter and round joins/caps. The root viewport at `map_render.rs:154` crops geometry natively; there is no endpoint culling or diameter normalization. The common display scale preserves aspect ratio (`:152`).
- Segment clipping at `map_render.rs:115` places pipe labels on a visible centerline segment even when both endpoints lie outside. Chamber labels account for visible centers, crossing boundaries, and a selection entirely inside a chamber (`:199`). Escaping applies to text and pipe titles (`support/map_svg.rs:5`, `:37`; `map_render.rs:145`).
- The legacy probe diff only imports and invokes the extracted geometry writer (`crates/simulation/examples/heart_preview_probe.rs:10`, `:53`). CSV columns/values, graph/Q computation, the three historical view rectangles, heart-domain/port marks, diagnostic captions, diagnostics-only return, movement, and guards are preserved. The helper retains the prior geometry, color, width, and Q-title serialization.
- The three concise example tests call real parse/render functions and inspect observable output (`map_render.rs:261`, `:289`, `:309`). They cover the approved key behavior without adding a duplicate shared-helper suite or a separate testing framework.

## Delivery and verification evidence

Read the recorded final results: `cargo test --locked -p simulation --example map_render` exited 0 with 3 tests passing; `cargo fmt --all --check` and `cargo check --locked --workspace --all-targets --features godot_bridge/lbm-gpu` exited 0. These commands were not rerun in this review.

Read `docs/research/2026-11-02-map-region-render/exports.txt`, `preview-checks.txt`, the research README, tool instructions, and STATUS changes. Read actual SVG content and viewed all three committed PNGs. The heart, microvascular, and small-world selections show the requested coordinate regions, proportional geometry, grid coordinates, and original widths. The images retain four chamber outlines, graded D24/D6/D3 pipes, and the small-world map's D7/D6.2 widths.

Read the actual scratch `tiny-crossing.svg`: the root selection is `-1 -65 2 2`; edge 223 retains the full centerline `-620,900 -700,900 -700,-64 38,-64` and diameter 24, with the label anchored at `(0,-64)` inside the visible crossing segment. This is direct output evidence for the crossing/crop requirement.

Read the actual legacy CSV and three scratch SVGs. The CSV retains its historical columns and reference Q48 data; the SVGs retain historical viewports, Q titles, captions, and heart-port/domain annotations. The reported diagnostics-only run returned before traversal. Source comparison confirms the compatibility boundary independently of that run.

The documented external PNG conversion uses existing system librsvg/cairo, adds no project dependency, and has a reproducible command. ImageMagick's remaining typography differences are disclosed. The archived output is clearly described as static geometry, without claiming flow, physical accuracy, gameplay, or performance acceptance.

## Issues

- Critical: none.
- Important: none.
- Actionable Minor: none.

## Accepted limits and declined to judge

- Simple label overlap and edge clipping: visible in the heart/small-world PNGs and expressly accepted/disclosed; automatic label packing is outside the approved scope.
- ImageMagick native-reader typography parity: disclosed external-tool limitation; the reviewed SVG and librsvg PNG evidence suffice, without a new raster backend.
- Exhaustive IEEE-754 extreme-range handling: `map_render.rs:152` can overflow its derived display scale for positive subnormal rectangle dimensions such as `1e-310`. This has no meaningful current-map selection use and was not promoted into a new robustness requirement under the project's proportional-reliability guidance. No extreme-input verification or numerical framework is requested by this review.
- Arbitrary externally supplied or malformed maps: the CLI only exposes two existing fixed factories; map import/registration and generalized geometry repair are outside the plan.
- Runtime flow/physics, full replay, engine/UI behavior, GPU/cloud/performance, and universal converter compatibility: outside this static example-tool change; none is inferred from compilation or images.

The review only read source and existing evidence and wrote this report. It did not rerun completed tests, builds, exports, XML/conversion checks, engine/GPU/replay/UI work, install software, dispatch agents, edit source/index/HEAD/branch state, push, or merge. Parent-owned plan/ledger closure remains with the parent.
