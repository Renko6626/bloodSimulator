# Full-detail cell count experiment

This is an independent copy of the cached Rapier/LBM experiment from
`docs/research/experiments/2026-10-23-lbm-scale`. It uses the same 12-node
soft body, layout-specific disk radius, `radius * 0.1` contact radius, mass 1,
material, damping, four markers per body, and initial velocity `(+0.05, 0)`.
The marker drag sums to `0.25` per body. Each Rapier substep freezes marker
geometry and prepares its stencil once; every LB step samples velocity and
recomputes force.

The default `expanded` layout keeps radius `0.075` for every count. The
physical height is 1. The width is `count / 16`, giving grid dimensions
`Nx = 32 * count / 16`, `Ny = 32`, and spacing `h = 1/32`. Each unit-wide tile
has cell centers at all combinations of x/y offsets `0.17, 0.39, 0.61, 0.83`.
This measures the cost of increasing cell count at constant cell size and
constant cell density while the fluid area grows.

The `packed` layout uses one `1 × 1` domain with `Nx = Ny = 32` at every count.
Its cell radius stays `0.025` for all cases. Counts 16, 32, 64, and 128 use
center grids 4 × 4, 8 × 4, 8 × 8, and 16 × 8 respectively, with x centers
evenly spaced from 0.05 to 0.95 and y centers from 0.1 to 0.9. This measures
capacity on a fixed fluid grid. Its cell diameter is about 1.6 fluid grid
spacings, so it is only a low-resolution point-drag approximation; do not
interpret it as a few-percent fluid accuracy result or a squeeze/blockage
validation. The two layouts have different geometry and should not be
compared as the same physical solution.

Both layouts use one fluid grid for all cells, with periodic x streaming and
fixed y walls. Rapier contact uses unfolded x coordinates, so x contact is
not periodic. Relaxation time is 2 and LB time step is 1/2048.

Run from the repository root (offline locked release build, then eight serial
runs pinned to one CPU):

```sh
python3 docs/research/experiments/2026-10-23-lbm-many/run.py --cpu 2 --seconds 6
python3 docs/research/experiments/2026-10-23-lbm-many/analyze.py
```

`manifest.json` records the layout, domain width, radius, and grid dimensions
for each case. `frames.csv` records physical and timing diagnostics every 1/60 s. The
nearest intercell node distance is calculated every frame for count 16. For
larger counts it is calculated on frame 1, every 30th frame, and the final
frame; `inf` marks frames where that optional O(N²) diagnostic was skipped.
`outlines.csv` samples every 0.1 s, or every 0.5 s for count 128, plus the
final frame. All finite-node, signed-area, fluid-density, kernel-weight,
clock, and exchange-impulse checks remain active every frame or LB step as
applicable. The timing of `diagnostics_ms` is therefore only directly
comparable across counts on matching sampled frames.

Performance summaries exclude the first 30 frames. Stability checks use all
360 frames per case. `whole_ms` includes diagnostic work but excludes output
I/O, initialization, rendering and game logic. A frame advances 1/60 model
second with two Rapier substeps and 34/35 LB steps; all cases finish exactly
12288 LB steps over six model seconds. Model units have not been calibrated
to the game's world and clock.

`results/environment.json` records the CPU, compiler and measured source
hashes. `results/baseline-equivalence.log` records that expanded-16 matches
all non-timing CSV fields of the previous 16-cell cached experiment. The
host is not exclusive and each case is one trajectory, so small timing
differences are not reliable optimization claims.

The formal run and plots are discussed in
[the report](../../2026-10-23-many-cells-and-lod.md). This standalone package
does not change the game's dependencies or fluid backend. The script uses
the repository target directory and the earlier experiment's executable
name; run the research build scripts serially.
