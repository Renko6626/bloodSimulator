# Single-cell LBM–Rapier dynamics probe

This is an isolated numerical spike, not a game backend or a calibrated cell model. It uses one Rapier `SoftBodyBuilder::disk` with the existing **12 boundary nodes**, nominal mass 1, membrane spring frequency 12 Hz, area spring frequency 30 Hz, linear damping 0.5, contact friction 0.12 and the existing ball surface collider pattern. The local unit-square fluid domain requires a smaller cell radius **0.15** and particle/contact radius **0.015**; the product uses radii 0.7 and 0.07. The mechanical mass remains the product's experimental parameter; it has not been reconciled with fluid displacement or cell physiology. This prototype's total coupling drag Γ=0.25 comes from the LBM four-point experiment; it is **not** the product's one-way `cell_drag=20` and the old velocity relaxation is not used.

The fluid is a 32², τ=2 D2Q9 model with x periodic and y bounce-back walls, background force zero. Each of four massless interior markers uses half of node 0/3/6/9 and half of the 12-node centroid for both position and velocity. The marker drag is Γ/4. Reaction impulse is returned using the transpose weights: half to its selected node and half spread evenly across all 12. This conserves the discrete exchange sum and matches the marker torque and instantaneous power for the frozen node state. Markers do not have separate mass or motion integration. The fluid spans the cell interior; its fluid mass and the Rapier body's nominal mass are both present. This is a permeable force-point experiment, not a no-slip membrane or a displacement-corrected cell.

Rapier runs at 1/120 model second, twice per 1/60 frame. Fluid lattice time is 1/2048. A single running target time determines the integer number of lattice steps each substep, so fluid never accumulates the error of `ceil(17.067)` at every substep. The cell state is held fixed during those lattice steps. All of their fluid reaction impulses are summed and distributed to Rapier nodes once before `world.step()`. Any external drive is added only as Rapier impulses. No position or velocity teleport is used.

## Scenarios

- `relax`: still fluid, cell center (0.5, 0.5), all mechanical nodes initially moving at x velocity 0.1, no external drive. Observe physical drag and damping decay.
- `reversal`: initially still cell and fluid at the same center; uniform external x force +0.08 for 0–1.5 s, −0.08 for 1.5–3 s, then zero. The force acts through the Rapier node masses. The cell moves across several 1/32 grid spacings. The applied force and exact time of reversal are recorded in the frame CSV.
- `squeeze`: initially still cell and fluid at the same center. Kinematic Rapier plates at y = 0.5 ± half-gap, half-gap 0.25 until 0.5 s, narrows to 0.12 by 1.25 s, holds through 1.75 s, and opens by 2.5 s. The plates are **contact proxies only**. They do not become LBM obstacles, so this case does not validate fluid walls or squeeze-film flow. The four interior markers stay away from the fluid's y walls. Contact-driven jumps should be interpreted separately from free-motion numerical jitter.

Default duration is 3 s per scenario. A longer 4–5 s run gives more post-drive/post-contact recovery time. Outputs in the chosen directory are one frame CSV plus node outlines every 0.1 s (with timestamps and plate y positions) and fluid-field snapshots every 0.5 s. The frame CSV includes center motion, finite-difference center acceleration, largest node displacement per frame, area error, density range, fluid speed, lattice step count, time offset, total fluid/body exchange mismatch and timing. The `drive_force_x` field is the actual frame-averaged force from the two substeps. `whole_ms` includes the fluid and Rapier updates plus frame diagnostics, and ends before CSV writing or optional snapshot export. Physical failure checks reject nonfinite nodes, inverted/zero area, nonpositive or nonfinite density/velocity, broken marker kernel weights or exchange, and clock offset at least one lattice time step. These checks do not imply that a few-percent accuracy target has been met; interpret actual measured maxima and time series.

## Reproduce

From this directory, with dependencies already cached:

```bash
cargo build --offline --release
taskset -c 3 target/release/lbm-rapier-dynamics-probe smoke 0.25
taskset -c 2 target/release/lbm-rapier-dynamics-probe results 4
```

The package has its own `[workspace]` and lockfile. The source imports `../../shared/fluid.rs` relative to `src/main.rs`. It does not edit the repository root manifest, lockfile, or product source.

Final six-second CPU 2 release runs are in `../results/rapier/`; the f32 comparison is in `../results/f32/`. See the parent archive README and Chinese report for the measured statistics.
