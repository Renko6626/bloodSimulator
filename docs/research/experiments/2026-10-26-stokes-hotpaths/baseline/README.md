# Profiling baseline

从 2026-10-26 Rayon 规则通道研究求解器复制的独立 crate，包名 `baseline_stokes_solver`。FFT、Thomas、Schur、marker CG、有限阻力及诊断算法均保持原样；只加可开关的分段时钟。`Stokes::new(n)` 是原串行路径，`Stokes::with_threads(n,0|1|2|4)` 与旧接口相同。

新增 `set_profiling(bool)` 与 `take_profile() -> Profile`。默认关闭；调用 `set_profiling` 时清零计数，`take_profile` 返回当前段累计值并清零，适合每个动态子步后取一次。`Profile` 有 `inverse_calls`、`velocity_inverse_ms`、`inverse_prepare_copy_ms`、`forward_fft_ms`、`mode_gather_solve_scatter_ms`、`inverse_fft_ms`、`pressure_stencil_ms`。其中 `velocity_inverse_ms` 包含它的四个子阶段；`pressure_stencil_ms` 只含 Schur 路径的梯度/散度 stencil，不含 CG 向量更新。不要将嵌套或不同口径的时钟相加当整步。原 `Solution` 的 `kernel_prepare_ms`、`coupled_solve_ms`、`diagnostics_ms` 仍在，且 `coupled_solve_ms` 包含速度逆和压力工作。

主性能测量应关闭 profiling；少量开启的帧只用于定位。构造固定 Rayon 线程池仍在 `Stokes::with_threads` 阶段，需与稳态求解时间分列。
