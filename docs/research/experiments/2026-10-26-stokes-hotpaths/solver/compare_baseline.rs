use baseline_stokes_solver::fluid as baseline;
use stokes_hotpath_solver::fluid as optimized;

fn inputs() -> (Vec<baseline::Marker>, Vec<optimized::Marker>) {
    let mut a = Vec::new();
    let mut b = Vec::new();
    for row in 0..4 {
        for column in 0..4 {
            let center = [(column as f64 + 0.5) / 4.0, (row as f64 + 0.5) / 4.0];
            let cell = row * 4 + column;
            let velocity = [0.08 + 0.003 * cell as f64, 0.01 * ((cell % 3) as f64 - 1.0)];
            for offset in [[0.045, 0.0], [0.0, 0.045], [-0.045, 0.0], [0.0, -0.045]] {
                let position = [center[0] + offset[0], center[1] + offset[1]];
                a.push(baseline::Marker {
                    position,
                    velocity,
                    drag: 0.0625,
                });
                b.push(optimized::Marker {
                    position,
                    velocity,
                    drag: 0.0625,
                });
            }
        }
    }
    (a, b)
}

fn compare(a: &baseline::Solution, b: &optimized::Solution) -> (bool, f64) {
    fn scan(a: &[f64], b: &[f64], same: &mut bool, max: &mut f64) {
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b) {
            *same &= x.to_bits() == y.to_bits();
            *max = (*max).max((x - y).abs());
        }
    }
    let mut same = true;
    let mut max = 0.0_f64;
    scan(&a.grid_velocity, &b.grid_velocity, &mut same, &mut max);
    scan(&a.pressure, &b.pressure, &mut same, &mut max);
    let fa: Vec<_> = a.marker_forces.iter().flat_map(|f| *f).collect();
    let fb: Vec<_> = b.marker_forces.iter().flat_map(|f| *f).collect();
    scan(&fa, &fb, &mut same, &mut max);
    let ja: Vec<_> = a.sampled_velocity.iter().flat_map(|f| *f).collect();
    let jb: Vec<_> = b.sampled_velocity.iter().flat_map(|f| *f).collect();
    scan(&ja, &jb, &mut same, &mut max);
    scan(
        &[
            a.divergence_rms,
            a.momentum_rms,
            a.force_law_rms,
            a.grid_force_balance_abs,
        ],
        &[
            b.divergence_rms,
            b.momentum_rms,
            b.force_law_rms,
            b.grid_force_balance_abs,
        ],
        &mut same,
        &mut max,
    );
    assert_eq!(a.fluid_calls, b.fluid_calls);
    assert_eq!(a.pressure_iterations, b.pressure_iterations);
    assert_eq!(a.coupling_iterations, b.coupling_iterations);
    (same, max)
}

fn emit(
    n: usize,
    threads: usize,
    profiling: bool,
    kind: &str,
    a: &baseline::Solution,
    b: &optimized::Solution,
    pa: baseline::Profile,
    pb: optimized::Profile,
) -> Result<(), String> {
    let (same, max) = compare(a, b);
    if !max.is_finite() || max > 1e-10 {
        return Err(format!(
            "{kind} n={n} threads={threads}: max difference {max:.3e}"
        ));
    }
    if profiling {
        if pa.inverse_calls == 0 || pa.inverse_calls != pb.inverse_calls {
            return Err(format!(
                "{kind} n={n} threads={threads}: profile inverse call count differs"
            ));
        }
        for (name, value) in [
            ("baseline inverse", pa.velocity_inverse_ms),
            ("optimized inverse", pb.velocity_inverse_ms),
            ("baseline copy", pa.inverse_prepare_copy_ms),
            ("optimized copy", pb.inverse_prepare_copy_ms),
            ("baseline forward FFT", pa.forward_fft_ms),
            ("optimized forward FFT", pb.forward_fft_ms),
            ("baseline modes", pa.mode_gather_solve_scatter_ms),
            ("optimized modes", pb.mode_gather_solve_scatter_ms),
            ("baseline inverse FFT", pa.inverse_fft_ms),
            ("optimized inverse FFT", pb.inverse_fft_ms),
            ("baseline pressure stencil", pa.pressure_stencil_ms),
            ("optimized pressure stencil", pb.pressure_stencil_ms),
        ] {
            if !value.is_finite() || value <= 0.0 {
                return Err(format!(
                    "{kind} n={n} threads={threads}: {name} profile={value}"
                ));
            }
        }
    } else if pa.inverse_calls != 0 || pb.inverse_calls != 0 {
        return Err("disabled profiling recorded inverse calls".into());
    }
    println!("{kind},{n},{threads},{profiling},{same},{max:.3e},{},{},{},{},{:.3e},{:.3e},{:.3e},{:.3e},{},{}",
        a.fluid_calls,b.fluid_calls,a.coupling_iterations,b.coupling_iterations,
        a.force_law_rms,b.force_law_rms,a.divergence_rms,b.divergence_rms,
        pa.inverse_calls,pb.inverse_calls);
    Ok(())
}

fn main() -> Result<(), String> {
    let (am, bm) = inputs();
    println!("kind,n,threads,profiling,arrays_and_residuals_bitwise_equal,max_abs_delta,baseline_fluid_calls,optimized_fluid_calls,baseline_coupling_iterations,optimized_coupling_iterations,baseline_force_law_rms,optimized_force_law_rms,baseline_divergence_rms,optimized_divergence_rms,baseline_profile_inverse_calls,optimized_profile_inverse_calls");
    for n in [32, 64, 128] {
        for threads in [0, 1, 2, 4] {
            for profiling in [false, true] {
                let mut a = baseline::Stokes::with_threads(n, threads)?;
                let mut b = optimized::Stokes::with_threads(n, threads)?;
                a.set_profiling(profiling);
                b.set_profiling(profiling);
                let sa = a.solve(&am, [0.4, -0.1])?;
                let sb = b.solve(&bm, [0.4, -0.1])?;
                emit(
                    n,
                    threads,
                    profiling,
                    "self",
                    &sa,
                    &sb,
                    a.take_profile(),
                    b.take_profile(),
                )?;
            }
        }
    }
    // Also exercise the prior-field path whose optimized implementation avoids a full copy.
    for threads in [0, 2] {
        for profiling in [false, true] {
            let mut a = baseline::Stokes::with_threads(64, threads)?;
            let mut b = optimized::Stokes::with_threads(64, threads)?;
            a.set_profiling(profiling);
            b.set_profiling(profiling);
            let prior_a = a.solve(&[], [1.0, 0.0])?;
            let prior_b = b.solve(&[], [1.0, 0.0])?;
            a.take_profile();
            b.take_profile();
            let sa = a.solve_lagged(&am, [1.0, 0.0], Some(&prior_a))?;
            let sb = b.solve_lagged(&bm, [1.0, 0.0], Some(&prior_b))?;
            emit(
                64,
                threads,
                profiling,
                "lagged",
                &sa,
                &sb,
                a.take_profile(),
                b.take_profile(),
            )?;
        }
    }
    Ok(())
}
