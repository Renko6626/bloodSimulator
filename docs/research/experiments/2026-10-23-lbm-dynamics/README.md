# LBM moving markers and one Rapier soft body

These are independent research programs, outside the product Cargo workspace. See the [Chinese report](../../2026-10-23-lbm-dynamics.md).

- [shared/fluid.rs](shared/fluid.rs): f64 dynamic-marker D2Q9 kernel.
- [shared/fluid_f32.rs](shared/fluid_f32.rs): same public interface, f32 lattice storage/arithmetic.
- [moving](moving/README.md): prescribed motion, grid crossing, periodic seam and frozen reference.
- [rapier](rapier/README.md): 12 mechanical nodes, four internal force sites, three six-second scenes.
- [analysis](results/analysis.json), [overview](results/overview.png), [squeeze animation](results/squeeze.gif).

Run from the repository root:

```bash
python3 docs/research/experiments/2026-10-23-lbm-dynamics/run.py --cpu 2 --seconds 6
python3 docs/research/experiments/2026-10-23-lbm-dynamics/run_f32.py --cpu 2 --seconds 6
python3 docs/research/experiments/2026-10-23-lbm-dynamics/plot.py
```

The runners overwrite their result files, use an allowed CPU affinity, and reuse the repository's ignored target cache. They do not change its manifest or lockfile. Rapier is pinned to 0.36.1 and built with `--offline --locked --release`. Plotting uses existing NumPy, Matplotlib and Pillow. The analysis expects the archived six-second/360-frame protocol. Source hashes and environment information are in `results/environment.json` and `results/f32/environment.json`.

The measured full-step costs include diagnostics but exclude CSV/snapshot output and rendering. Timings discard the first 30 frames; stability checks retain all frames. Each scene is one trajectory on a nonexclusive host; results do not establish multi-cell or target-player performance. The moving-driver elapsed time includes per-step CSV output and is not a fluid performance measure.

The squeeze plates participate only in Rapier contacts. They are not LBM obstacles. The body is permeable and its nominal mass is uncalibrated against the full-domain fluid. Model time has not been mapped to the game's world scale.
