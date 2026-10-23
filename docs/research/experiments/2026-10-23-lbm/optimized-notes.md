# Bounded CPU optimization of the archived LBM spike

`optimized.rs` is copied from `docs/research/experiments/2026-10-23-lbm/main.rs` and archived as a separate experimental variant. It remains std-only, f64, with the same D2Q9 BGK/Guo equations, tau, physical time mapping, steady stop, marker force update on every LBM step, CLI and CSV schema. No product code was edited; the archived baseline is retained.

At initialization, `stream_targets` enumerates fluid sites and precomputes each of their nine streaming destinations, including periodic x and halfway link bounceback. It asserts each fluid destination population has exactly one writer. This permits `next[dst][q] = post` and removal of full-grid `next.fill` each step. Solid populations are never read. Reciprocal tau and the Guo prefactor are computed once. Stencil interpolation, collision arithmetic and physical diagnostics otherwise follow the baseline.

Build with `rustfmt --check optimized.rs` and `rustc -O optimized.rs -o lbm-opt`. Validation compiled the archived baseline separately and ran both on CPU 3 for straight32 tau1, channel32 tau1, blob4 32 gamma .25 tau1, and blob4 32 gamma .25 tau2, each with two 10-step post blocks. All non-timing summary fields matched exactly; all 1024 exported field rows in every case matched exactly (maximum absolute difference zero). `*-base-result.csv`, `*-opt-result.csv`, corresponding fields and post samples are retained here. Timing fields were excluded from equality checks.

The single CPU 3 smoke showed lower settle and post time, but this nonexclusive one-run observation is not a performance distribution. A serialized repeated-block benchmark is required for a timing claim. This optimization does not reduce the number of LBM steps per physical frame or test moving geometry.

Final serial measurements and exact numerical-equivalence checks are now in `results/optimized/`; run `python3 run_optimized.py --cpu 2 --blocks 30` from this directory after the baseline runner. See the main experiment report for final timing.
