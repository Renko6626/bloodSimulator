# Sparse blob resistance in a MAC Stokes fluid

This independent numerical probe extends the 2026-10-22 circular-marker experiment. It retains the same standard-library-only Rust code, four-point Peskin interpolation/spreading, and complete divergence-free Stokes solve `K`. No product code, Cargo dependency, Rapier body, membrane mechanics, or dynamic motion is added.

The unit square is periodic in x and has no-slip walls at y=0,1. Viscosity is 1 and background body force is zero. Every prescribed marker has `V=(0.1,0)`. `--shape blob` places either one point at `(0.5,0.5)` or four points on the cardinal directions at radius 0.075. The points are fluid-coupling sites, **not** mechanical degrees of freedom. They do not enclose or exclude fluid; there is no blob interior boundary or penetration diagnostic. Circle mode retains radius 0.15 and its separate dense-circle reference diagnostic.

For blob mode, `--drag GAMMA` is a **total discrete resistance** divided evenly among the `m` points: `F_i=(Γ/m)(V_i-J_i u)`. Thus a one-point and a four-point test share the same nominal total Γ. Circle mode uses `--kappa KAPPA`, the old resistance **per arc length**, with `F_i=κ(2πR/m)(V_i-J_i u)`. These parameter values have different weights and cannot be directly compared numerically. Both modes solve `u=KSF` and the fixed-geometry dense system `[I/r + JKS]F=V`, where `r=Γ/m` or `κ 2πR/m`. `S` spreads total force to MAC faces and `J` samples face velocity. The lagged row instead applies `F_i=rV_i` once and reports its actual force-law residual; it is not a self-consistent solution.

Compile from this directory:

```bash
rustfmt --check main.rs separable.rs
rustc -O main.rs -o marker-probe
```

Representative runs; redirect each output to a separate CSV to retain the header:

```bash
taskset -c 3 ./marker-probe 32 1 results --shape blob --drag 1 > results/blob_n32_m1_g1.csv
taskset -c 3 ./marker-probe 32 1 results --shape blob --drag 4 > results/blob_n32_m1_g4.csv
taskset -c 3 ./marker-probe 32 1 results --shape blob --drag 16 > results/blob_n32_m1_g16.csv
taskset -c 3 ./marker-probe 32 4 results --shape blob --drag 1 > results/blob_n32_m4_g1.csv
taskset -c 3 ./marker-probe 32 4 results --shape blob --drag 4 > results/blob_n32_m4_g4.csv
taskset -c 3 ./marker-probe 32 4 results --shape blob --drag 16 > results/blob_n32_m4_g16.csv
taskset -c 3 ./marker-probe 64 4 results --shape blob --drag 4 > results/blob_n64_m4_g4.csv
taskset -c 3 ./marker-probe 32 24 results --shape circle --kappa 32 > results/circle_n32_m24_k32.csv
```

The self row uses `2m+1` complete fluid calls: one for each mobility column and one final solve. `assemble_ms` includes those `2m` solves; `factor_ms` includes the dense system construction and Cholesky solve; `final_fluid_ms` is the last Stokes solve. `total_ms` for self starts before grid/backend/marker setup and includes mobility assembly, antisymmetry check, factorization, and final solve. It excludes **post-solve** diagnostics and field export. The lagged row reuses the already-created grid/backend/markers after self and its `total_ms` is only its one **warm update** solve plus spreading. Do not divide self cold total by lagged warm total as a production speedup. Moving geometry would require rebuilding `JKS`, which cannot be treated as free.

`slip_rms` and `force_law_rms` are RMS across the `2m` velocity components, not RMS of per-point vector magnitudes. In particular, `force_law_rms` is the velocity-unit residual `RMS(F_i/r + J_i u - V_i)`; divide it by `|V|=0.1` if a relative closure error is needed. `max_sampled_speed_over_target` is the largest marker speed magnitude divided by `|V|=0.1`; it is not a global maximum over the fluid grid. `reaction_x/y` are the negative summed marker forces, and `force_balance_abs` checks them against the integrated fluid forcing. `div_rms` and `momentum_rms` check the final fluid solve. The relative antisymmetry of `JKS` is reported; the `1e-6` rejection threshold is only a gross-error guard, not proof of roundoff symmetry. Actual force-law residual and final fluid residuals judge the solution. `normal_abs_integral` and `tangent_abs_integral` are `NaN` in blob mode because the blob defines no surface.

Two CPU3 smoke runs at `n=32, Γ=4` gave:

| m | mode | slip component RMS | force-law component RMS | reaction Fx | max sampled speed / target | complete fluid calls | cold self / warm lag ms |
|---:|---|---:|---:|---:|---:|---:|---:|
| 1 | self | 0.030387 | 9.8e-18 | -0.171896 | 0.5703 | 3 | 2.57 |
| 1 | lagged | 0.023122 | 0.09383 | -0.400000 | 1.3270 | 1 | 0.81 |
| 4 | self | 0.035178 | 9.4e-15 | -0.198834 | 0.5230 | 9 | 7.87 |
| 4 | lagged | 0.003196 | 0.07173 | -0.400000 | 1.0566 | 1 | 0.79 |

For both self runs, integrated action-reaction mismatch was at most `5.6e-17`, divergence RMS at most `3.2e-11`, and momentum RMS at most `8.9e-15`. The lagged slip can look smaller while its force law is violated: here it overshoots the prescribed marker speed. Timings are exploratory single cold runs on a nonexclusive machine. The experiment demonstrates local entrainment and a finite-resistance self-consistent solve in this flat channel. It does not demonstrate volume blocking, no-slip behavior, moving-cell stability, narrow-gap accuracy, or frame-time performance.
