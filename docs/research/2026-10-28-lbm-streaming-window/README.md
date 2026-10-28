# LBM streaming window probe

**Current next steps (2026-10-28):** investigate prescribed-velocity boundary reconstruction through gradient-based regularization (FD), followed by non-equilibrium extrapolation (NEQ/eNEQ). See the [session note](boundary-next-steps.md) for scope and the minimal comparison sequence; no repair has yet been validated.

**2026-10-28 hardware update:** the reviewed production probe was run on RTX 3060. All ten cases failed before completing the hold; no migration occurred and no performance/profile acceptance is available. A fresh independent Sol high investigator reproduced the failure without mechanics and confirmed an unstable prescribed-velocity outlet mode at the selected low lattice viscosity. See the [hardware record](hardware-rtx3060-20261028/README.md), [independent causal investigation](hardware-rtx3060-20261028/independent-investigation.md), and [verified stop state](hardware-rtx3060-20261028/final-stop-status.json). The production solver was not modified by that investigation.

The production simulation runs a closed-loop straight or fork fixture, with a robot and 0 or 72 total cells, Fine or Coarse. A small subset compares outer bands 2/4 and fork distances 8/16 from the window's right opening. World window size is 48×48, block size is 16×16. Pressure is the game configuration default, 12, with its ordinary pulse and default velocity bound 10. The 72 cells are distributed around the loop; `active_cells` counts the maximum with valid local marker support in a substep.

Each case holds the robot near its spawn by ordinary movement input during warmup and the measurement interval. The window must not shift during this interval: if it does, the case fails. The geometry remains fixed, so the opening-to-fork distances remain 8 or 16 throughout the hold. After the hold, 16 frames of rightward input and eight post-movement frames capture one warmed migration, including the pre-migration rows. The input controller and robot coupling are part of this workload. Timing p95 values use hold frames only; migration rows have separate phase labels and work counters. `region_switches` is cumulative.

`functional.csv` and companions are an **incomplete, failing** short-horizon diagnostic dataset: two warmup plus four hold frames and up to 24 migration/post frames on llvmpipe. The fork/Fine cases failed after the shift at default pressure; an isolated follow-up also fails while held, before any migration. Eight other cases completed. These results do not support hardware performance or shipping acceptance. The probe continues after individual case errors but returns a nonzero exit code and reports every failure; the failing frame has no production CSV row. The `status.csv` sidecar records completion, completed frame count and error per case. For the existing pre-fix dataset, the sidecar was reconstructed from the captured rows and failure log; later runs write it directly. Preserve stderr with any run.

`functional.csv.migration.csv` exports the first eight cached post-shift substep deltas against the **pre-shift baseline**, including frame and calculated substep end time; the historical committed CSV predates a timestamp correction, so its final entry can be one substep late when the eight-entry cap is reached in a frame’s first substep; they include ordinary fluid evolution and are not migration-only errors. The `immediate_center_*` and `overlap_error` fields in the main CSV describe the immediate overlap copy, not later evolution. No extra GPU readback is issued for these observations. `functional.csv.reference.csv` compares zero-cell Coarse public StreamingWindows: the moving window follows the robot's previous-frame position while the reference remains fixed. Both receive the same graph flow and reach equal physical time; neither includes robot coupling, and the shift can lag production by one frame. Thus the reference is not a cell-loaded production comparison.

The sampled fork/Fine instability exceeds the configured speed bound: in a held run cached-grid max speed increased from 8.26 at frame 10 to 14.94 at frame 12 and 142.60 at frame 15, followed by `invalid fluid state` at frame 16. This lower-bound speed scan uses the public cache sampler, not a GPU readback. The robot remained near the vessel center; migration is not necessary for this reproduction. The underlying numerical cause remains unconfirmed. The default small_world is unsupported. Fine viability, fallback, buffer, transition width, strip warmup, and long-term behavior are unresolved.

The CSV density sum is dimensionless and resolution-dependent, not physical mass. Full-step timing surrounds `try_tick` and excludes the subsequent snapshot, Godot bridge and rendering. All timings are CPU wall measurements; the LBM stage includes waiting, GPU kernel time is unavailable, and stages overlap. Never add their independent p95 values.

## Source bundle and proposed hardware command

The representative-pressure fork/Fine case is unsupported. A reviewed hardware run may measure completed Coarse cases and observe Fine failure; it cannot establish a shipping profile without real data. These commands do not start an instance or read credentials. On the reviewed clean source checkout, create an archive and a separate manifest; transfer **both** files by the authorized method:

```sh
set -euo pipefail
git diff --quiet
git diff --cached --quiet
git rev-parse HEAD > /tmp/lbm-window-source-commit.txt
git archive --format=tar HEAD | gzip > /tmp/lbm-window-source.tar.gz
```

On the execution machine, with both files in the current directory, extract to an empty directory and retain the manifest alongside the results. A source archive has no `.git`; do not use `git rev-parse` there. Supply the source archive and manifest paths as appropriate for the transfer.

```sh
set -euo pipefail
mkdir -p lbm-window-source results/lbm-window
tar -xzf lbm-window-source.tar.gz -C lbm-window-source
cp lbm-window-source-commit.txt results/lbm-window/source-commit.txt
cd lbm-window-source
cargo build --locked --release -p simulation --features lbm-gpu --example lbm_window_probe
timeout 3600 target/release/examples/lbm_window_probe --warmup 20 --frames 720 --output ../results/lbm-window/hardware.csv 2>&1 | tee ../results/lbm-window/run.log
```

The 720-frame hold spans 12 game-time seconds. As a planning scale, the straight fixture's nominal flow of approximately 18.5 across a 6-unit opening gives mean speed approximately 3.09 units per game second, so 8 and 16 units correspond to roughly 2.6 and 5.2 seconds of advection. Fork speeds and transient propagation differ; this estimate is not proof the boundary influence has reached every sampled location. Review measured flux and the full time series before drawing a buffer conclusion. The binary rejects software adapters in hardware mode. Record GPU/driver and command exit status; timeout or a failed case invalidates acceptance of that case; the status sidecar identifies completed cases. The command reports separate nearest-rank p95 values for hold-frame full step, coupling, wait-inclusive LBM stage and Rapier.
