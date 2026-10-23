# LBM–Rapier scale and marker-cache probe

This is an isolated numerical and density/scale experiment, not a product backend. Its `src/main.rs` owns the multi-cell Rapier harness; `../../shared/fluid.rs` owns the lattice and prepared-marker API. The original research archive, product, root manifest, and root lockfile are untouched. Build uses the copied standalone `Cargo.lock` and cached Rapier 0.36.1.

## Physics and scenes

All runs use one unit-square 32² D2Q9 fluid (τ=2, x-periodic, y bounce-back) and one global lattice step over **all** markers. Each Rapier disk has 12 nodes, mass 1, 12 Hz membrane, 30 Hz area, damping 0.5 and contact friction 0.12. Four markers per cell combine half a selected node (0/3/6/9) and half its centroid for position and velocity. Total Γ is 0.25 **per cell**, shared equally across markers. The reaction impulse uses the transpose weights and original f32 node accumulation order. The global fluid clock has 1/2048 s steps; the solid has two 1/120 s steps per 1/60 s frame.

`relax`, `reversal`, and `squeeze` are reference scenes with one radius 0.15 cell and particle radius 0.015, matching the archived dynamic experiment. `reversal` applies +0.08 x force until 1.5 s, −0.08 until 3 s, then zero. `squeeze` uses moving Rapier contact plates from the archived scene; they are not fluid walls.

`scale` has 1/4/16 cells of **fixed radius 0.075**, particle radius 0.0075, each initialized at +0.05 x velocity. The 1/4/16 layouts have center 0.5; a symmetric 2×2 grid at 0.35/0.65; and a 4×4 grid at 0.17/0.39/0.61/0.83. Initial disks do not overlap. Radius stays fixed across counts to isolate the cost of adding cells at this scale. This is a separate density/scale experiment; its physical results cannot be compared directly with the archived radius 0.15 cell.

`counterflow` uses the 16-cell layout and alternates initial column velocity +0.05/−0.05, with no continuing drive. It was added to produce potential cell contacts; the uniform `scale` drift is not evidence of cell-cell collisions.

`baseline` rebuilds marker kernels on every LB step. `cached` prepares all frozen markers once per Rapier substep, then reuses the support weights across LB steps. The fluid still interpolates current grid state and recomputes force each LB step. Both modes use the same scene, stepping and return weights.

Markers must retain complete kernel support inside the y walls; the fluid reports `WallSupport` instead of trimming or clamping. x coordinates stay unwrapped for Rapier and wrap only when indexing fluid. Rapier has no periodic contact across the x seam, so later seam-crossing cells need separate interpretation.

## Outputs and timing

Each invocation writes `frames.csv` and `outlines.csv` in `--outdir`. Every frame records aggregate center motion, largest node speed/jump, smallest cell area, largest cell-relative area change, fluid density/speed/mass, global LB step count and clock mismatch, actual grid-versus-body impulse mismatch, contact counters, and timing. `outlines.csv` samples all nodes of all cells every 0.1 s and at the last frame.

`solver_contacts_sum` and `solver_contacts_max` come from Rapier's public `world.physics_pipeline.counters.solver.ncontacts` after each solid substep. They count contacts across the entire world, possibly including self contact or plate contact; they are **not** specifically cell-cell counts. `closest_intercell_node_distance` measures node-to-node distance only, not outline clearance. For one cell it is infinity. Contact interpretation should combine the solver counters, scene geometry and outlines.

Timing fields are milliseconds per frame: `prepare_ms`, `fluid_ms` (LB update, including force interpolation/scatter/evolution and exchange capture), `node_transfer_ms` (impulse distribution and substep setup), `rapier_ms`, `diagnostics_ms`, and `whole_ms`. `whole_ms` ends before CSV writes. A few clocks bracket each substep phase; no clocks run inside each LB step. `whole_ms` can exceed the sum of listed phases due to loop and bookkeeping overhead. Per-frame timing is exploratory on a shared CPU; compare repeated pinned release runs for a performance claim.

## Commands

From this directory:

```bash
cargo build --offline --release

# CPU 3, short smoke; not a formal timing sample.
taskset -c 3 target/release/lbm-rapier-scale-probe --outdir results/smoke-scale16-baseline --mode baseline --scene scale --count 16 --duration 0.25
taskset -c 3 target/release/lbm-rapier-scale-probe --outdir results/smoke-scale16-cached --mode cached --scene scale --count 16 --duration 0.25

# Six-second reference trajectories in both modes.
for scene in reversal squeeze; do
  for mode in baseline cached; do
    taskset -c 3 target/release/lbm-rapier-scale-probe --outdir "results/${scene}-${mode}" --mode "$mode" --scene "$scene" --count 1 --duration 6
  done
done

# Six-second 1/4/16-cell scaling trajectories.
for count in 1 4 16; do
  for mode in baseline cached; do
    taskset -c 3 target/release/lbm-rapier-scale-probe --outdir "results/scale-${count}-${mode}" --mode "$mode" --scene scale --count "$count" --duration 6
  done
done

# Approved contact-producing input, cached mode.
taskset -c 3 target/release/lbm-rapier-scale-probe --outdir results/counterflow-16-cached --mode cached --scene counterflow --count 16 --duration 6
```

For an exact mode comparison, compare the CSV columns other than `mode` and timing; compare `outlines.csv` after dropping `mode`. The first 15 frames of short `reversal` and `squeeze` runs also match every shared numerical field of the archived radius 0.15 CSV at its exported precision.
