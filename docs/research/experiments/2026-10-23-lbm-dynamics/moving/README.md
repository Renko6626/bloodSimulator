# Prescribed moving-marker LBM probe

The driver links `../shared/fluid.rs` directly. It uses a fixed 32² domain, τ=2, ν=ρ=1, `dt=1/2048`, periodic x and halfway bounce-back at y walls. Four markers sit 0.075 from a prescribed center; their drag coefficients sum to Γ. `smooth` uses `x=0.5+0.2 sin(πt)` (four seconds); `seam` uses `x=0.55+0.1t` (five seconds). The center's x coordinate remains unwrapped for the body and is wrapped only when sampling the fluid.

Build and run (all measurements here used CPU 3; the printed elapsed time includes per-step diagnostics and CSV output, so it is not a fluid-only performance benchmark):

```bash
rustc --edition 2021 -O -Awarnings moving/main.rs -o moving/moving
taskset -c 3 moving/moving smooth 0.25 4 moving/smooth-g025.csv
taskset -c 3 moving/moving seam 0.25 5 moving/seam-g025.csv
taskset -c 3 moving/moving smooth 4 4 moving/smooth-g4.csv
taskset -c 3 moving/moving frozen 0.25 1.044921875 moving/frozen-module.csv
```

All traces contain each LB step's commanded speed, fluid maximum speed, density range, mass drift, marker/fluid exchange impulse, kernel weight error, force change, commanded speed change, grid crossings, center and marker seam crossings, and periodic distance to the seam. Each run also writes a final cell-center field in `.field.csv`.

| Case | Steps | Density range | Max speed | Max mass drift | Max exchange error | Max kernel weight error |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| smooth Γ=.25 | 8192 | .999839700–1.000159040 | .040127632 | 2.280e-13 | 6.099e-20 | 4.441e-16 |
| seam Γ=.25 | 10240 | .999974374–1.000025879 | .006639359 | 2.854e-13 | 9.317e-21 | 4.441e-16 |
| smooth Γ=4 | 8192 | .997899635–1.002071069 | .340000723 | 2.274e-13 | 5.963e-19 | 4.441e-16 |

After the first 0.1 model second, the largest one-step force changes at grid crossings versus other steps were `2.179e-4` vs `2.273e-4` for smooth Γ=.25, `1.151e-6` vs `2.046e-6` for seam Γ=.25, and `1.914e-3` vs `1.943e-3` for smooth Γ=4. The smooth command itself changes each step; near its turn at t=.5, commanded speed change is about `9.64e-4` per step. The larger all-time off-grid jumps (`7.855e-4`, `1.250e-4`, `2.011e-1`) occur immediately after starting from still fluid, not at grid crossings. These observations show no crossing-specific force spike in the tested prescribed motions; they do not establish free-cell stability.

In the seam trace, the right marker crosses at t=3.75 with force change `1.180e-9`; the unwrapped center crosses x=1 at t=4.5 with force change `1.378e-11`. The trace records center x=1, wrapped x=0, periodic marker distance 0, and finite density/impulses at that step.

Frozen reference: compiled the repository's `optimized.rs` and ran `blob4`, n32, τ2, Γ=.25, speed .1, stopping at 2140 steps. The shared module ran the same 2140 steps. Comparing all 1024 exported cell-center values gives max absolute difference **0** for `u`, `v`, and density at the 12-digit CSV export precision. This checks that moving-stencil code at fixed positions preserves the old kernel's numerical result. The old reference reached its steady stopping condition at 2140 steps.

The archived CPU 2 run is reproduced by the parent `run.py`; final results are in `../results/moving/`. Its frozen comparison uses 3190 steps to match the earlier archived settle-plus-30-block field. Trace time labels describe the input at each step start; fluid diagnostics describe the state after that step.
