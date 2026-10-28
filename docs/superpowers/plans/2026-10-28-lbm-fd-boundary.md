# FD gradient opening experiment implementation plan

> **For agentic workers:** Use superpowers:executing-plans. This is an explicitly authorized research spike; project rules override mandatory TDD, additional agents and full-suite reviews.

**Goal:** Determine whether finite-difference reconstruction removes the observed outlet instability while retaining prescribed opening velocities and discrete fluxes at current parameters.

**Architecture:** Isolated copies of the existing Rust/wgpu/WGSL research harness; no production API, geometry, dependency or default changes. All nine opening populations are reconstructed with the supplied velocity, density inferred from outgoing information, and non-equilibrium stress from interior gradients. This retains interior response rather than resetting the opening to an analytic equilibrium field.

**Tech stack:** Existing D2Q9 MRT + Guo, Rust, WGSL, NumPy/SciPy diagnostics; local llvmpipe if hardware is absent.

**Spec/context:** `docs/research/2026-10-28-lbm-fd-boundary/sources/boundary-next-steps-context.md`; current opening-numerics spec and archived independent investigation.

## Constraints and hypotheses

- Preserve Fine tau=.50285715, Coarse tau=.50125, nu=.05, world spacing, timestep and all MRT rates. No clamp, feedback, NEQ, CBC, fixed-density replacement or geometric expansion.
- Primary density closure is Latt et al. (2008) Eq.27: `(A+2B-Fn/2)/(1-un)` from pre-reconstruction known populations. Their BC4 uses this closure too; retaining the formula is a testable hypothesis, not an assumption that density is stable. Full FD changes its coupled timestep operator.
- Secondary **predeclared density sensitivity only if needed**: first-interior density (`rho_b=rho_1`), still imposing all velocity components. It is pressure extrapolation, not rho=1; no outgoing non-equilibrium is extrapolated. Report separately; no parameter sweep.
- Normal derivative in lattice units: `(-3 ub+4 u1-u2)/2` in inward direction. Tangential derivative: centered differences of supplied boundary velocities; at opening endpoints use second-order one-sided differences within the opening. For a two-cell opening only, first difference. Explicitly check both inward-normal fluid neighbors per opening cell before execution. Existing diagonal geometry can lack this stencil despite an along-vessel straight buffer; report unsupported without expanding geometry.
- Read fully streamed interior populations to compute velocity with Guo half force. Boundary-to-boundary stencil reads use immutable prescribed velocities, avoiding read/write races in reconstruction.
- MRT stress hypothesis (leading low-Mach CE, not proven high-order MRT boundary theorem): `d=dx ux+dy uy`, `e=dx ux-dy uy`; `Pxx=-rho/3*(d/se+e/snu)-ux Fx`, `Pyy=-rho/3*(d/se-e/snu)-uy Fy`, `Pxy=-rho/(3 snu)*(dy ux+dx uy)-(ux Fy+uy Fx)/2`, with `se=1.1`, `snu=1/tau`. Trace and deviatoric rates are distinct.
- Rebuild shifted `gq=feq_q-wq + 4.5 wq Qq:P -1.5 wq cq.F`; hence sum(f)=rho and sum(c f)=rho ub-F/2 exactly in algebra. The source correction supplies both first and second moments; validate numerically with a bounded forced check. Pure-fluid runs have F=0 and cannot validate coupled forcing.
- FD stress is pre-collision: coefficient is `1/s`, never `tau-.5` (the latter belongs to physical viscosity). Higher kinetic moments follow the second-order projection; bulk MRT unchanged.

## Files and responsibilities

- `docs/research/2026-10-28-lbm-fd-boundary/src/lbm/`: copied numerical module; experimental pad modes only.
- `src/eigen.py`: exact Fourier-strip linear timestep operator, spectrum and mode artifacts.
- `src/eigen_gpu.rs`: same-mode WGSL comparison, total disturbance norm and macroscopic state.
- `src/diag.rs` and optional `src/pure.rs`: straight/fork no-mechanics diagnostic using established fixtures.
- `run.sh`, `results/`, `README.md`: reproducible commands, fresh build/source manifests, raw logs and scoped conclusions.

## Execution

- [x] Read AGENTS, context, investigation, opening specs, and primary literature. Isolate branch/worktree.
- [x] Reproduce archived baseline growth and construct FD linear operator. Check all 17 unique transverse Fourier modes for nx=32, ny=32 at Fine/Coarse (small 288x288 operators, one BLAS thread), including total spectrum rather than the old eigenvector alone.
- [x] Rebuild isolated WGSL harness against existing dependency cache. Compare baseline and FD seeded mode for 100 steps, reporting projection, total population disturbance and density. If FD has a new unstable eigenmode, seed that too. Stop the candidate if predicted/observed instability persists; bounded density sensitivity is allowed, no unbounded tuning.
- [x] Only promising candidates progress to current no-mechanics straight Fine/Coarse pulsed 160-frame runs; sample bad cells every lattice step and flux/velocity/rho/mass at frames. Then established fork/diagonal cases if straight remains bounded. No mechanics, migration, clouds or performance claims.
- [x] Archive observations and exact commands, checks and limitations. Update checkboxes and commit only experiment-owned files; no merge.

## Review focus / stopping criteria

A decaying projection is insufficient: inspect full spectra and total disturbance. Correct opening flux is insufficient: inspect rho, mass and interior speed. Exact local moments do not guarantee global mass conservation. Failure/nonpositive rho/nonfinite state ends that case; growth exceeding unity in a new strip mode blocks promotion unless plainly a neutral density mode within roundoff. If both predeclared density closures fail, stop FD development and report rather than change tau or add fixes. A 160-frame software hold is functional evidence only, never hardware acceptance or proof for unsupported junction openings.

## Execution ledger

- Research/plan completed before numerical edits; main agent read the concrete plan and confirmed scope.
- Full strip spectra and fresh WGSL baseline/FD runs completed on llvmpipe. No extra unstable strip mode found in the sampled operator.
- Four real-domain no-mechanics cases completed160frames; this is a finite hold, not acceptance. rho_1 not promoted beyond spectral sensitivity.
- Diagonal stencil unsupported at wall endpoints. No geometry change, fallback or fluid run.
- Ruling: explicit experiment authorization covers these numerical choices; no repeated architecture approval or extra agents. No production API/dependency/boundary change.
- Validation scope: existing research diagnostics plus source/style/manifest checks, no permanent test additions or full workspace rebuild.
- Scope correction: one duplicate final archive execution occurred after formatting to align binary provenance; it added no cases or acceptance requirement. The lead instructed no further numerical reruns; none followed.
- Final review: self-review of source, spectra, logs and bounds; no additional agent was authorized. No merge/push.
