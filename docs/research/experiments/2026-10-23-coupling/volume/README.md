# Brinkman MAC/Stokes numerical spike

This is an isolated, steady, dimensionless numerical experiment. It is not a game solver or a model of a deformable cell. `main.rs` reuses the prior MAC grid and `separable.rs` constant-coefficient velocity inverse. Rust standard library only.

## Equations and geometry

The domain is `[0,1]²`, periodic in x with no-slip y=0,1. MAC velocities occupy x/y faces; pressure occupies cell centers. The solved system is

`B u + G p = rhs`, `D u = 0`, `B = -Δ + κχ`, `rhs = f + κχV`.

Both face masks use the solid indicator evaluated at their own face centers. This is a binary, stair-step volume mask, with no cut-cell fraction. The original separable `A⁻¹` is used **only as a PCG preconditioner** for `B`; pressure Schur applications each call that `B` solve. All outputs report the residual of the actual `B` equation after the final solve.

- `channel`: lower wall `y_b(x)=0.12+0.04 sin(2πx)` and upper wall `y_t(x)=0.88−0.08 sin(2πx)`. The solid regions outside these curves have `V=0`; `f=(1,0)` everywhere. The outer box walls retain their original no-slip conditions. This is a smooth periodic contraction, not a branching vessel.
- `disk-static`: solid disk centered at `(0.5,0.5)`, radius `0.15`, `V=0`, with `f=(1,0)` everywhere. It is an impermeable-body *approximation* with finite permeability, not a membrane.
- `disk-moving`: the same disk with prescribed `V=(0.1,0)`, `f=0`. It is one instantaneous steady snapshot. There is no position update, acceleration, or fluid time history.

The penalty length is `ell_p=1/sqrt(κ)` (viscosity `μ=1`), and `ell_over_h=ell_p/h`. At fixed resolution, smaller `ell_over_h` means stronger penalization. Sweeping `ell_over_h` across resolutions changes the physical κ. A separate fixed-κ comparison is needed to study spatial refinement.

## Build and runs

```bash
rustc -O main.rs -o mac-stokes-b
./mac-stokes-b --n 32 --scene channel --ell-h 2
./mac-stokes-b --n 32 --scene disk-static --ell-h 1
./mac-stokes-b --n 32 --scene disk-moving --ell-h 0.5
./mac-stokes-b --n 32 --scene channel --kappa 1024
./mac-stokes-b --n 64 --scene channel --kappa 1024 --field channel-64.csv
./mac-stokes-b --n 64 --scene disk-static --kappa 1024 --field disk-static-64.csv
```

`--n` must be a power of two. Run each process serially for meaningful timing. Each invocation prints a CSV header and one result row. `solve_ms` includes pressure and velocity iterations, final velocity recovery and solver allocations. It excludes mask/preconditioner setup, diagnostics, CSV output, and process startup. No warm start is used.

`mom_rms` is RMS of `B u + G p − rhs` on all active velocity faces; `div_rms` is RMS of `D u` on all cells. `tang_rms` and `normal_rms` interpolate face velocities to 256 points per true wall curve (or disk circumference), subtract the prescribed boundary velocity, and integrate squared tangential and normal components by physical arclength. `abs_normal_flux=∮|(u−V)·n|ds`; its sign is deliberately discarded, since signed flux can cancel. Interpolation is bilinear and the geometric wall is not aligned to the binary mask.

`full_q_min/max` integrate x velocity through the entire box at each MAC x-face, including the penalized region. They should agree up to discrete divergence error. `fluid_q_min/max` exclude solid-masked x-faces; variation is expected because the finite-κ model permits crossing the curved wall or disk. Do not interpret the constant full-box flux as conservation inside a physical vessel. For the moving disk, the same caveat applies to flux relative to the moving boundary.

`reaction_x/y` and `torque` sum `κχ(u−V)h²` over their respective staggered velocity faces; torque uses the 2D moment around `(0.5,0.5)`. These are volume reactions per unit out-of-plane depth, not resolved surface traction. For `disk-moving` with zero forcing, the x reaction should oppose the prescribed positive x motion. No analytic force truth is claimed for this finite periodic channel.

In the static scenes, the background force also acts inside the penalized region, so this reaction includes that region's body-force contribution. It must not be interpreted as a pure surface drag measurement. The `fluid_q` variation also contains binary-mask sampling error; for a moving body it includes the changing excluded cross section. Use the true-boundary relative normal diagnostics to discuss penetration.

The field CSV records cell-center interpolated velocity, pressure and geometric solid indicator. It is for inspection and plotting; the equation is solved at staggered faces, not at these exported cell centers.

This spike does not test a thin membrane, marker force transfer, Rapier response, moving-domain time stepping, or 5 ms full game-step performance.
