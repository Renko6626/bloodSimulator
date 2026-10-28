# Independent investigation of the RTX 3060 LBM failures

Investigator: fresh independent Sol agent; 2026-10-28. Read-only production investigation; no production fix, parameter/default change, permanent test, cloud access, SSH, secret access, or subagent was used. Temporary diagnostic sources and logs are in `/tmp/lbm-independent-sol-20261028/`.

## Conclusion

**Confirmed causal defect: the prescribed-velocity outlet reconstruction, combined with the selected low lattice viscosity/MRT relaxation, has an unstable transverse boundary mode.** Small disturbances grow at the outlet even when the prescribed velocity and integrated opening flow remain correct. They eventually produce negative density. This is a numerical boundary/configuration compatibility failure, not evidence of a broken RTX 3060, a force-unit error, or a necessary dependence on robot/cell coupling, pulsing, migration, or a fork.

The reconstruction implements the approved algebra correctly. Conserving density/momentum and exactly matching boundary velocity does not establish stability. The production timestep selector limits lattice velocity and an upper viscosity scale, but does not establish stability of this outlet operator near `tau=0.5`. The research implementation used periodic x boundaries and therefore did not exercise this outlet operator.

The cause is established by three independent observations: production-API reproductions at the same failing frames as hardware after removing all mechanics; single-variable outlet controls; and a predicted unstable eigenmode whose growth is quantitatively reproduced by the production WGSL collision/reconstruction in a diagnostic domain.

## Source and raw evidence

- Hardware source: `7727c605c6000dd0e3ad6cbf413b3b17f4f3c1c4`; inspected HEAD `00f9d33` has no `crates` difference from that source (`git diff --stat 7727c605c6000dd0e3ad6cbf413b3b17f4f3c1c4 HEAD -- crates` was empty).
- Immutable evidence root: `docs/research/2026-10-28-lbm-streaming-window/hardware-rtx3060-20261028/`.
- Independently recomputed all **18** hashes in `results-manifest.sha256`: all matched. `results/manifest.json` and `results/binary-manifest.json` identify source and binary. Binary SHA-256: `b8820925769fa307b77b41a5fd168effc7b658cb92db259fdb7d8446cc827cf3`.
- `results/command.txt`: `lbm_window_probe --warmup 20 --frames 720 --output results/hardware.csv`.
- `results/hardware.csv.status.csv:2`–`:11` and `results/run.log:1`–`:11`: all ten cases failed with production `invalid fluid state`. Completed frames: straight Fine 17/18 (0/72 cells), straight Coarse 43/43, fork Fine 16/16, fork Coarse 35/35; band-2 straight Coarse 43, offset-8 fork Coarse 37.
- Parsed all **303** frame rows: adapter exclusively `NVIDIA GeForce RTX 3060`, `software=false`, `region_switches=0`. No measured case completed, so no accepted performance p95 exists. Completed frames are not exact partial-frame failure times.

Observed raw density growth, straight Fine/0 cells: frame 0 `[0.9999633,1.0000348]`; frame 12 `[0.98490345,1.0050203]`; frame 15 `[0.9331558,1.1175835]`; frame 16 `[0.7685454,1.2761078]`. At frame 16, targets `[-21.044168,21.044168]` and observed opening fluxes `[-21.04417,21.044168]` still agree. Correct flux is therefore not a stability certificate.

## First bad state and propagation

`crates/fluid-gpu/src/lbm/lbm.wgsl:83`–`:88` computes the boundary density from known populations and prescribed velocity, then reconstructs three incoming populations. The state is used in the next collision (`:15`–`:22`). In the reproductions, growing disturbances localize at outlets before negative density appears there or immediately inside them. `crates/fluid-gpu/src/lbm/gpu.rs:559` rejects nonpositive/nonfinite fluid density; `crates/simulation/src/fluid/streaming_window.rs:156`–`:160` checks after the lattice batch, and runtime propagates this as the observed advance error.

The raw hardware dump contains only completed-frame diagnostics and cannot establish its first bad cell or lattice step. The exact cells/steps below are **local llvmpipe functional observations**, not claims of hardware substep timestamps.

## Minimal production-API reproduction

`pure.rs` calls the existing production `rasterize`, `opening_profiles`, `GraphNetwork` and `LbmWindow` APIs. Its map, initial flow, profile, `nu=0.05`, timestep/count, and framewise pressure are those of the hardware probe. It never creates a `Simulation`, Rapier world, robot, cells, coupling kicks, or migrating window. It reads the field after every lattice step. Thus the solver/geometry path remains real production logic while the mechanical systems are absent.

Build, from the project root:

```sh
rustc --edition=2024 -O /tmp/lbm-independent-sol-20261028/pure.rs \
  -L dependency=target/release/deps \
  --extern blood_fluid_gpu=target/release/deps/libblood_fluid_gpu-2c48bc0f8179894e.rlib \
  --extern simulation=target/release/deps/libsimulation-b9a5802a0c6a13dc.rlib \
  -o /tmp/lbm-independent-sol-20261028/pure
```

Runs/results (all exited after printing the first invalid cell):

| Command suffix after `/tmp/lbm-independent-sol-20261028/pure` | First invalid step, zero-based frame/substep | First invalid cell; world position | Density |
|---|---|---|---|
| no suffix | 252; frame 17, k=13 | `[191,92]`; `[47.875,23.125]`, right outlet | `-0.14467096` |
| `coarse` | 347; frame 43, k=2 | `[94,49]`; `[47.25,24.75]`, right outlet neighbor | `-0.5695952` |
| `fork` | 227; frame 16, k=2 | `[124,191]`; `[31.125,47.875]`, top outlet | `-0.10531056` |
| `fork coarse` | 283; frame 35, k=2 | `[68,94]`; `[34.25,47.25]`, top outlet neighbor | `-0.10798383` |
| `frozen` (straight Fine, fixed pressure 12) | 256; frame 18, k=3 | `[191,92]`; `[47.875,23.125]` | `-0.10345173` |

Logs: `pure-fine.log`, `pure-coarse.log`, `pure-fork-fine.log`, `pure-fork-coarse.log`, `pure-fine-frozen.log` in the temporary directory. In all four pulsed zero-cell cases, the failing frame equals the corresponding real-GPU frame despite removing all mechanics. Frozen pressure also fails; pressure pulsing is not required.

The failing Fine straight disturbance grows at the right outlet, away from wall endpoints: frame 10 maximum speed `5.4853`, frame 12 `7.6216`, frame 14 `18.5006`, frame 16 `70.0098`. The original imposed maximum remains about 5 world units/s. The later velocity-bound violation is an effect of the unstable mode; it is not an oversized imposed velocity at initialization.

## Units and reference comparison

The following conversions are consistent with the intended route:

- `lbm_geometry.rs:65`–`:69`: planar parabolic velocity `3Q/(4RD)` with `D=1`; initial and discrete-normalized opening profiles differ slightly because only the latter is normalized exactly on the grid. That small mismatch can seed disturbances but is not required for instability (see the uniform-flow mode below).
- `gpu.rs:704` and `:783`–`:786`: world velocity to lattice velocity uses `dt/h`.
- `gpu.rs:670`: `tau = 0.5 + 3 nu dt/h²`; acceleration uses `dt²/h`, readout uses `h/dt` (`:665`–`:672`). This is also the research formula.
- Default Fine: `h=.25`, `dt=.0011904762`, 7 lattice steps/mechanical substep, `tau=.50285715`; Coarse: `h=.5`, `dt=.0020833334`, 4 steps, `tau=.50125`.
- In the straight fixture, initial peak lattice speed is about `.02205`; the pulsed peak is about `.02536`. Both are below the selector's `.05` bound. A small Mach number does not remove the unstable boundary mode.
- `docs/superpowers/specs/2026-10-28-lbm-opening-numerics.md`: for inward normal `n`, the production `rho`, `kn`, `kt`, tangent difference and three writes match the specified formula, including shifted populations and force half-step signs. No algebraic discrepancy was found.
- Production MRT collision coefficients/matrix match the research implementation. The material difference is the boundary: `docs/research/2026-10-27-ns-lbm/lbm/src/lbm.wgsl:44`–`:47` always wraps x periodically; production `lbm.wgsl:62`–`:88` adds fixed-velocity openings. The earlier periodic benchmark does not validate this closure.

## Discriminating boundary controls

`diag.rs` uses a copied production `lbm` module under the temporary directory, with the same straight 48×48 world window, Fine grid, initial profile, fixed pressure 12, viscosity, lattice dt, and collision. Q is `18.529297`, the production initial straight rate. Diagnostic mode is passed in otherwise-unused `Params.pad`; mode zero follows the production equations. Only temporary copies were modified.

Build (also used for `eigen_gpu.rs`, replacing input/output names):

```sh
rustc --edition=2024 -O /tmp/lbm-independent-sol-20261028/diag.rs \
  -L dependency=target/release/deps \
  --extern wgpu=target/release/deps/libwgpu-5a7ee29f61bc02e0.rlib \
  --extern bytemuck=target/release/deps/libbytemuck-40ae7fd0291dbddd.rlib \
  --extern pollster=target/release/deps/libpollster-17ad20f61a6070c7.rlib \
  -o /tmp/lbm-independent-sol-20261028/diag
```

| Command | Sole numerical difference | Result |
|---|---|---|
| `LBMDIAG=0 /tmp/lbm-independent-sol-20261028/diag frozen` | none | negative rho at step 256, `[191,92]`; reproduces production API frozen run |
| `LBMDIAG=1 /tmp/lbm-independent-sol-20261028/diag frozen` | periodic x streaming; no x reconstruction, wall bounceback retained | all 2240 steps complete; final rho `[.9999987,1.0000008]` |
| `LBMDIAG=2 /tmp/lbm-independent-sol-20261028/diag frozen` | right outlet only: set rho=1 and derive normal momentum; keep inlet velocity, all collision parameters unchanged | all 2240 steps complete; final rho approximately `[1,1.0002033]` |

The pressure-outlet control sets `rho=1`, `kn=rho-A-2B` at the right boundary (zero force), then uses the same three reconstruction equations. It removes fixed outlet velocity. This is a causal control, **not a proposed drop-in production fix**, because it relaxes the current exact opening-flow contract.

## Quantitative mechanism: unstable outlet eigenmode

An independent algebraic linearization in `eigen.py` uses the exact D2Q9 velocities, weights, orthogonal MRT matrix/norms, relaxation rates `[0,1.1,1,0,1.2,0,1.2,1/tau,1/tau]`, streaming, and the production opening formula. Orthogonality `M Mᵀ=diag(norm)` is checked. For equilibrium flow `(U,0)` and zero force:

1. `delta u = delta j - u delta rho` at rho=1.
2. Differentiate the same second-order equilibrium; collide with `I - M^-1 S M (I - d(feq)/d(f))`.
3. Stream with transverse Fourier phase `exp(-i ky cy)`.
4. Reconstruct the same three incoming populations at each x boundary using prescribed velocity; `delta rho=(delta A+2 delta B)/(1-u_n)`.

This yields a small exact linear timestep operator for a uniform strip. With `nx=32`, periodic transverse extent 32, `U=.022`, actual Fine `tau=.502857142857`, and `ky=2*pi*13/32`, its dominant eigenvalue is **1.0593634892428312**. The mode peaks at the outlet; outlet/inlet amplitude ratio is approximately 348854. It is a transverse boundary disturbance growing about 5.94% per step, even at a low prescribed lattice speed.

The differentiable model alone is not accepted as proof. `eigen_gpu.rs` injects this eigenvector at amplitude `1e-6` into the **copied production WGSL**. Diagnostic changes are a 32×32 grid, periodic y, full-height planar x openings (corner-range validation bypassed only for this periodic domain), population injection/readback, and the mode selector. Collision and the fixed-velocity reconstruction for `LBMDIAG=3` remain the production equations. A uniform initial flow removes walls, forks, profile mismatch, forcing and pressure pulsing.

Commands:

```sh
OPENBLAS_NUM_THREADS=1 python3 /tmp/lbm-independent-sol-20261028/eigen.py
LBMDIAG=3 /tmp/lbm-independent-sol-20261028/eigen_gpu
LBMDIAG=4 /tmp/lbm-independent-sol-20261028/eigen_gpu
```

Results:

| Outlet | Initial projected amplitude | Step 50 | Step 100 |
|---|---:|---:|---:|
| production prescribed velocity | `1.0000042e-6` | `1.7873052e-5` | `3.194823e-4` |
| rho=1 pressure control, otherwise identical | `1.0000042e-6` | `4.4339114e-7` | `2.0936251e-7` |

Observed per-step growth: `(3.194823e-4 / 1.0000042e-6)^(1/100) = 1.05936214`, matching the independently predicted `1.05936349`. This establishes that the implemented boundary/collision operator amplifies disturbances; roundoff or initial-profile errors merely seed them.

Additional linear controls at `ky=pi/2`: U=0 gives spectral radius `.99705796`; U=.022 with the production tau gives `1.00691636`; replacing only the outlet by pressure gives `.99548472`; changing only tau to `.55` gives `.95050451`. These are explanatory bounded controls, not a viscosity recommendation or an exhaustive stability envelope.

## A tempting local repair that did not work

For a further targeted control, the right-outlet reconstructed non-equilibrium distribution was projected onto second-order stress while preserving its computed density and prescribed velocity. This is a **local** stress projection using the outlet's own reconstructed populations, not an interior extrapolation scheme.

- `LBMDIAG=6 .../eigen_gpu`: the unstable-mode projection still grows from `1.0000042e-6` to `5.6503137e-5` in 100 steps.
- `LBMDIAG=5 .../diag` with the original pulse still fails at step 354 / frame 25, negative rho at `[191,92]`.

Therefore simply adding this local regularization delays failure and is not a supported fix. No production version of it was added.

## Smallest recommended repair and remaining uncertainty

The repair target is the **outlet closure plus its numerical operating envelope**, principally `lbm.wgsl:62`–`:88`. Keep Rust/wgpu, D2Q9 MRT/Guo, geometry, coupling, and streaming interfaces. Do not lower pressure until one example happens to pass, add clamping to hide negative density, or rewrite the mechanical system.

There is not yet an experimentally validated repair that also preserves the approved exact-flow condition at every opening. The smallest next repair experiment should replace only the unstable outlet closure with a stable prescribed-flow/outflow treatment and first pass the seeded eigenmode test at the existing tau/U. A method extrapolating boundary non-equilibrium from interior cells is a candidate requiring numerical design review, **not a verified recommendation to implement as-is**. The local projection tested above must not be substituted for such a design.

If the product can accept boundary flow responding to pressure instead of exact graph-prescribed outlet flow, a fixed-density outlet is the smallest demonstrated stabilizing numerical change. It is a boundary-contract decision requiring user confirmation; the 2240-step control is not flow/accuracy/game acceptance. For a multi-outlet fork it also changes flow allocation, so cannot silently replace the approved design.

Remaining limits:

- Four no-mechanics production-path cases reproduce hardware failure frames, but exact hardware first-bad cell, substep and eigenmode amplitude are unavailable. Different timing in other coupled/offset cases is not evidence of hardware failure.
- The independent spectrum is for a uniform strip, not the full fork Jacobian. The production-map reproductions provide the link to actual cases; they do not exclude additional coupling or junction issues after this defect is removed.
- A passing boundary control does not validate the full 720-frame run, migration, cell coupling, or performance. No extra real-GPU run is needed to identify this cause. After an approved outlet repair, the smallest hardware confirmation is the existing zero-cell Fine straight and Fine fork cases with per-lattice-step bad-cell capture and a bounded 160-frame hold, then the existing full matrix only if those remain valid.
- Software timings are not hardware performance measurements.

## Temporary artifacts and preservation

All scripts, copied modules, binaries and logs remain in `/tmp/lbm-independent-sol-20261028/`; the only repository write by this investigator is this report. Production/raw/Git state was preserved. Final source digests:

```text
ba7afcbd55c630297ec4ef56fecd4329dc4356319ce2c1bb333b71591d5a2539  pure.rs
104b53adc5f1ed057541d29b8a7f25bc0ab689bafe18e2e9888a2bc38a8d291c  diag.rs
c1b1cf07a87ef41e2e608a34734633dafd247422fa505d39c83613cc6b7dc5c9  eigen.py
40d10887c386fcc994c9b464d6d764fb92b0c5be5e9d21b298581d564df5261b  eigen_gpu.rs
d4777d3272a53355bcca7b5ef2ec534f7dc6afe94a57c169beef62730e067857  crates/fluid-gpu/src/lbm/lbm.wgsl
785986f1b086530e32a73d4b5aaadcdf8deae12e5024ad3c9ded317fbd2792b6  crates/fluid-gpu/src/lbm/gpu.rs
74b6eee6152460bec0b524803e6fb5bc817c8d14d86b565b4d5dfa0c9b71dc48  crates/fluid-gpu/src/lbm/mod.rs
```
