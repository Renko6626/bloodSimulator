# LSQ-FD budget benchmark and10000-frame follow-up

User approved benchmark first, then one GPU10000-frame paired experiment. Main agent alone owns cloud/SSH/run/stop; this worker owns new research harness/analysis. Preserve `fork_a Fine`192²,14lattice/gameframe, MRT+Guo/LSQ/nu/dt/profiles, and current frame-end equilibrium-difference correction. No optimization, new numerics, geometry matrix or production change.

## Benchmark

Two serial, fresh single-window modes: A uncorrected, B mean-controlled. Each100warmup+500measured frames. Initialization/shader compilation/weight loading and warmup excluded. For each frame: prepare/upload profile; enqueue7steps; field readback+finite check; enqueue7steps; A field readback, or B raw9-population readback→CPU f64 density-onlymean→unchanged normalizationpass→field readback; finite check. End wall timing only after readback and validation complete. Do not use paired windows or per-lattice field checks for performance.

Record totalwall avg/median/p95/max and16.6667ms budget fraction, plus profile, each7-step CPU enqueue, each fieldread wall, raw read wall, CPUmean, normalization enqueue, validation, and existingwork_stats readwait/decode/upload/encode/submit/counts/bytes. Enqueue is CPU submission duration, notGPU duration; no timestampfeature added. Stagepercentiles overlap/asynchronous and must not be added. B−A is combined readback/CPUmean/normalization overhead, not purekernel cost. The measurement excludes mechanics, graphsolve, migration, rendering and is not complete-gameFPS.

Copy currentmodule into new researchdir and add statistics to existing raw-read/normalization helper only; WGSL remains byte-identical. CPUmean sums densities only, with positivity check. One local smoke `--warmup0 --frames1` runs1frame per mode (2total), then freezebinary/sourceSHA.

## Long run

Main agent runs the old frozen `mean_density` binary SHA2c451ec2b3090af4db77f98184a8c26fb20bde76c3a3fc28195d97032ca7351e with `--frames10000`, one same-device baseline/control pair, preserving all per-step validity/momentlimits/field comparisons and thresholds. No oldfile/manifest changes; no additionalforce run besides the old harness's existing one. Invalid state/localcheck failure stops normally; comparisonguardrail crossings are retained and reported. No parameter adjustment or rerun.

Analyze whole-run meanresidual, baseline drift, gross/netcompensation, velocity/demeanedrho/neqstress global+center relativeL2 and absolute trends/peaks, and limit crossings. Mean=1 is designed behavior; removal of periodic globalmean pressure and lack of physical baseline truth remain limitations.

## Files and completion

New `docs/research/2026-10-29-lbm-budget-longrun/`: benchsource/copiedmodule, build/analysis, benchmarkCSV and metadata, GPUraw/lifecycle (main-agent owned), longrun outputs and sourceSHA. Existingweight reused. No permanenttests/fullworkspacechecks.

- [x] Implement/format/build and <=2-frame local smoke; freeze and hand off before report writing.
- [x] Main-agent GPUbenchmark→oldbinary10000pair once; retrieve/verify before stopping.
- [x] Analyze stage budgets and long trends, concise report/source checks, confirm stopped/exited and commit experimentonly.

Execution: local1frame per mode (2total) only; frozen benchmarkbinary SHA9c6920144d24cd02af9d7de68d12127350966ce2472d0ddf10c5371143029ac2. Main agent ran GPUbenchmark then unchanged oldbinary10000pair once, retrieved27 results, confirmed58133143 stopped/exited. All old1000 CSVprefixes match. Field guardrails remained unchanged; no additional numerical run or optimization.
