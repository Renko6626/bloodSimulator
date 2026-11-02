# Task 1 review

Reviewed range: `bf44817..4647d6d` in `feat/map-region-render`.

**Spec verdict: PASS. Quality verdict: PASS.** No Critical, Important, or actionable Minor findings. Task 1 can proceed to the parent's documentation closure and final branch review.

## Requirements and implementation

- Read the task brief, global constraints, review instructions, approved design, implementation report, packaged diff, and actual changed source. The change stays within the two examples, their example-only helper, documentation, and archived output; no production map, solver, dependency, or public interface changes.
- `crates/simulation/examples/map_render.rs:36` consumes signed numeric values directly and rejects non-finite numbers. Rectangle validation at line 58 rejects non-positive dimensions and unrepresentable endpoints; required output/rectangle and fixed map choices are checked at lines 69–104. Help, unknown arguments, write errors, and nonzero failure exit are straightforward (`:231`, `:246`).
- The new command directly constructs only `MapDefinition::small_world` or `MapDefinition::heart_preview` (`map_render.rs:236`). Inspected their existing source-backed factories and rendering calls. There is no Simulation, GraphNetwork, physics-world, Godot, LBM, or traversal creation in this command. The existing Rapier Vector value used for chamber containment is not a world construction.
- `examples/support/map_svg.rs:13` preserves chamber boundary points and full centerlines, actual diameter `half_width * 2`, rounded joins/caps, and legacy colors. The new root viewport clips this full geometry without endpoint culling (`map_render.rs:154`), and width/height use one common scale (`:152`). Crossing pipes therefore retain their true width.
- Label clipping computes a midpoint on an intersecting centerline segment (`map_render.rs:115`, `:139`, `:190`), including segments with both endpoints outside. Chamber names handle a visible center, boundary, or an entirely contained viewport (`:199`). Text/title escaping is centralized (`map_svg.rs:5`, `:37`; `map_render.rs:145`).
- Grid placement starts at `ceil(start / step) * step`, so it remains aligned to the world origin (`map_render.rs:173`); parsing bounds both density and line count (`:95`). Grid and descriptive labels are optional and do not imply flow direction or speed.
- The legacy probe diff is confined to importing and calling the extracted writer (`heart_preview_probe.rs:10`, `:53`). CSV generation, graph/Q values, all three rectangles, domain/port annotations, diagnostics-only return, movement, and guards remain intact. The writer preserves their serialized geometry and Q titles.

## Evidence inspected

- Read archived `exports.txt` and `preview-checks.txt`, the README's actual commands/source provenance, and the final reported Cargo results: example tests 3 passed, fmt passed, and workspace all-target feature compilation passed. These completed commands were not rerun during review.
- Directly inspected the SVG root/viewBox, actual polygon/polyline data, widths, coordinate labels, and final annotations for heart, micro, and small-world artifacts; viewed all three committed PNGs. Their proportions and displayed geometry agree with the requested coordinate regions. The examples retain D24/D6/D3 widths and the existing small-world D7/D6.2 widths rather than normalizing lines.
- Inspected scratch `tiny-crossing.svg`: viewBox `-1 -65 2 2`, edge 223 retains its full centerline `-620,900 -700,900 -700,-64 38,-64` and width 24; its emitted label anchor is `(0,-64)`, inside the rectangle while the horizontal segment endpoints lie outside.
- Inspected actual legacy scratch CSV and three SVG files: the flow CSV retains its existing columns and Q48 reference data; SVGs retain the three historical viewports, Q titles, diagnostic captions, and heart-port/domain annotations. The diagnostics-only path still returns before movement.
- The three short example tests exercise production parse/render functions and observable SVG output (`map_render.rs:261`, `:289`, `:309`); there is no duplicated helper suite or source-string inspection.

## Accepted limits and verification scope

Simple label overlap and edge clipping are visible and documented, consistent with the explicitly accepted absence of label packing. The existing external ImageMagick reader's typography differences are documented; the archived PNGs use already installed librsvg/cairo solely for evidence. Neither issue calls for a new renderer or dependency.

This review was read-only apart from this report. It did not rerun completed checks/exports/conversions or start engine, GPU, browser interaction, replay, cloud, or performance work. No additional acceptance beyond static map export is claimed.
