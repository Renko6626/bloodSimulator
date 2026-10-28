# LBM streaming window probe

The probe runs the production simulation with a robot in a closed loop. The matrix covers straight and fork geometry, Fine and Coarse, 0 and 72 total cells, plus Coarse cases for a 2-unit transition band and an opening 8 rather than 16 units from the fork. The window is 48×48 and blocks are 16×16. The feature distance is measured from the initial right window edge to the fork on the centerline; motion changes this distance. The starting robot is at x=31.98 and ordinary rightward movement crosses a block boundary. The 72 cells are distributed along the loop; `active_cells` is the actual maximum number with valid marker support in a substep.

`functional.csv` contains three frames per case on llvmpipe. `functional.csv.reference.csv` compares two zero-cell public StreamingWindows, with a fixed window and one moving by one block according to the robot's previous-frame position. Both receive the same graph flow and advance to the same physical time; these reference windows do not include the robot's coupling. Their migration can occur one substep/frame later than the production window. The center comparison samples their common valid region. At 0.05 game-time seconds all reported reference deltas were zero; this short horizon does not establish transient or buffer adequacy. The first frame has no movement input to capture the pre-migration force; subsequent frames use ordinary movement. `region_switches` is cumulative. Per-frame marker-force values allow comparing the migration frame and subsequent frames, but a three-frame check is not a stability assessment.

The software adapter is **not** a hardware measurement. Fine viability, Coarse fallback, 2 versus 4 transition width, 8 versus 16 buffer distance, strip warmup, and long-term migration transient remain pending real hardware and a long enough physical horizon for boundary influence. The fixtures use pressure 0.01 and do not validate the default small_world, which remains unsupported. The captured lattice density sum is dimensionless, not physical mass; it is resolution dependent. Wait-inclusive CPU stage timings and work counters are not GPU kernel times; constituent durations overlap and cannot be added to their enclosing stages or their p95 values.

## Reproduce on authorized hardware

Use a reviewed committed source revision, Rust and a supported GPU driver. These commands only build and run locally; they do not start a cloud instance or read credentials. The timeout bounds the run, and a timeout or error leaves an incomplete CSV that must not be treated as a result. Increase the frame count for the propagation horizon when the physical travel time warrants it; 120 measured frames alone does not establish it.

```sh
set -o pipefail
mkdir -p results/lbm-window
git rev-parse HEAD > results/lbm-window/source-commit.txt
git status --short > results/lbm-window/source-status.txt
cargo build --locked --release -p simulation --features lbm-gpu --example lbm_window_probe
timeout 3600 target/release/examples/lbm_window_probe --warmup 20 --frames 120 --output results/lbm-window/hardware.csv 2>&1 | tee results/lbm-window/run.log
```

The binary rejects software adapters in this mode. Record the exact command, GPU/driver and exit status alongside the output; the CSV records adapter and measurement mode. The summaries print independent nearest-rank p95 values from measurement frames for full step, coupling, wait-inclusive LBM stage and Rapier. They are not summed. Include warmup rows when examining migration. The companion reference CSV reports zero-cell same-time differences, subject to the scheduling and coupling limitations above.

For a clean source bundle after committing the probe, run `git archive --format=tar HEAD | gzip > /tmp/lbm-window-source.tar.gz`; transfer the archive by the separately authorized method and retain the recorded commit and commands with the results. Archived instance identifiers do not establish current availability or price.
