# Substep marker preparation and 1/4/16-cell scale study

Standalone research only; no product workspace change. Read the [Chinese report](../../2026-10-23-lbm-scale.md).

- [Fluid module](shared/fluid.rs): `prepare_markers` once per frozen solid substep; `advance_prepared` still recomputes fluid-dependent forces each lattice step.
- [Exact kernel comparison](shared/check_prepared.rs): calls the prior archived implementation and the new implementation, including marker motion across the periodic seam.
- [Harness and scenes](rapier/README.md): reference cells of radius .15; all scale counts use fixed radius .075.
- [Independent analysis](results/analysis.json) and [overview](results/overview.png).

From the repository root:

```bash
python3 docs/research/experiments/2026-10-23-lbm-scale/run.py --cpu 2 --seconds 6
python3 docs/research/experiments/2026-10-23-lbm-scale/repeat16.py
python3 docs/research/experiments/2026-10-23-lbm-scale/plot.py
```

The runner builds the independent package with cached Rapier 0.36.1 (`--offline --locked --release`) and stores hashes/environment metadata. It overwrites its result files and uses the project's ignored target cache. The repeat script records two extra run pairs for the specifically noisy 16-cell comparison; their physical outputs are checked against the initial run, then duplicate outlines are removed. Original repeat timing CSVs are retained. Plotting uses the existing NumPy and Matplotlib environment.

Every trajectory lasts six model seconds / 360 frames. Timing excludes its first 30 frames; stability analysis uses all frames. The main 16-cell chart point pools three runs per mode; 1 and 4 cells have one run per mode. p95 is nearest-rank, and the host is nonexclusive. The analysis script expects this archived protocol.

The baseline's marker preparation happens inside `fluid_ms`; compare `prepare_ms+fluid_ms` or `whole_ms`. Whole-step timing includes research node-distance diagnostics, not rendering or file output. Contact counters are full-world solver counts; one-cell intercell distance is undefined and is represented as infinity. The fluid is permeable to the cells, and Rapier has no periodic contact across x.
