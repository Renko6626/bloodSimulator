# Coupling research artifacts, 2026-10-23

These are standalone Rust experiments, outside the Cargo workspace. They add no game behavior or dependencies. Read the [Chinese report](../../2026-10-23-coupling-experiment.md) for the conclusions and their limits.

- [volume](volume/README.md): Brinkman penalty in a curved channel and around fixed/prescribed-motion disks.
- [markers](markers/README.md): one/four resistance points and a finite-resistance circular membrane reference.
- [volume raw samples](results/volume-samples.csv) and [summary](results/volume-summary.csv).
- [marker raw samples](results/markers-samples.csv) and [summary](results/markers-summary.csv).
- [independent statistics check](results/analysis.json), [figure PNG](results/overview.png), [figure SVG](results/overview.svg).

Run serially from the repository root:

```bash
python3 docs/research/experiments/2026-10-23-coupling/run_volume.py --runs 20 --cpu 2
python3 docs/research/experiments/2026-10-23-coupling/run_markers.py --runs 20 --cpu 2
python3 docs/research/experiments/2026-10-23-coupling/plot.py
```

The runners use `rustc -O`, build executables into temporary directories, pin the measured process to CPU 2, and overwrite their result CSVs. Choose an allowed CPU on another machine. Each case has one discarded warmup invocation and 20 recorded invocations. The host was not exclusive. Environment JSONs record the compiler, hardware, and measured Rust-source SHA-256 values. Plotting uses existing NumPy and Matplotlib. The plot script checks this archived 20-repeat protocol.

Timings have different meanings: volume `solve_ms` excludes geometry/backend setup; marker self `total_ms` includes setup, mobility assembly and the final solve; marker lagged `total_ms` is a warm one-fluid-update measurement reusing the initialized backend. They are not whole game-step costs. The archived p95 convention is `sorted[ceil((N-1)*0.95)]`, which is the maximum observation at N=20; use these as exploratory data, not a tail-latency guarantee.

Fields in `results/fields/` are actual numerical solutions. Marker field exports are the **self-consistent reference**, not the lagged approximation. Neither probe integrates a free body over time. They do not establish Rapier coupling stability, an impermeable cell, 72-cell performance or a 5 ms game step.
