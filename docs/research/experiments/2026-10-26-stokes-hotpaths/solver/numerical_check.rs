use stokes_hotpath_solver::fluid::{Marker, Solution, Stokes};

fn markers() -> Vec<Marker> {
    let mut out = Vec::with_capacity(64);
    for row in 0..4 {
        for column in 0..4 {
            let center = [(column as f64 + 0.5) / 4.0, (row as f64 + 0.5) / 4.0];
            let cell = row * 4 + column;
            let velocity = [0.08 + 0.003 * cell as f64, 0.01 * ((cell % 3) as f64 - 1.0)];
            for offset in [[0.045, 0.0], [0.0, 0.045], [-0.045, 0.0], [0.0, -0.045]] {
                out.push(Marker {
                    position: [center[0] + offset[0], center[1] + offset[1]],
                    velocity,
                    drag: 0.0625,
                });
            }
        }
    }
    out
}

fn compare(reference: &Solution, other: &Solution) -> (f64, f64, f64, f64, bool) {
    fn diff(a: &[f64], b: &[f64]) -> (f64, bool) {
        assert_eq!(a.len(), b.len());
        (
            a.iter()
                .zip(b)
                .map(|(x, y)| (x - y).abs())
                .fold(0.0_f64, f64::max),
            a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()),
        )
    }
    let (du, bu) = diff(&reference.grid_velocity, &other.grid_velocity);
    let (dp, bp) = diff(&reference.pressure, &other.pressure);
    let f0: Vec<_> = reference.marker_forces.iter().flat_map(|f| *f).collect();
    let f1: Vec<_> = other.marker_forces.iter().flat_map(|f| *f).collect();
    let (df, bf) = diff(&f0, &f1);
    let s0: Vec<_> = reference.sampled_velocity.iter().flat_map(|v| *v).collect();
    let s1: Vec<_> = other.sampled_velocity.iter().flat_map(|v| *v).collect();
    let (ds, bs) = diff(&s0, &s1);
    (du, dp, df, ds, bu && bp && bf && bs)
}

fn main() -> Result<(), String> {
    let points = markers();
    println!("n,threads,max_grid_delta,max_pressure_delta,max_force_delta,max_sampled_delta,arrays_bitwise_equal,serial_divergence_rms,parallel_divergence_rms,serial_momentum_rms,parallel_momentum_rms,serial_force_law_rms,parallel_force_law_rms,serial_fluid_calls,parallel_fluid_calls,serial_coupling_iterations,parallel_coupling_iterations,serial_pressure_iterations,parallel_pressure_iterations");
    for n in [32, 64, 128] {
        let reference = Stokes::new(n).solve(&points, [0.4, -0.1])?;
        for threads in [1, 2, 4] {
            let solution = Stokes::with_threads(n, threads)?.solve(&points, [0.4, -0.1])?;
            let (du, dp, df, ds, bits) = compare(&reference, &solution);
            let max_delta = du.max(dp).max(df).max(ds);
            if !max_delta.is_finite() || max_delta > 1e-10 {
                return Err(format!(
                    "n={n} threads={threads}: field delta {max_delta:.3e} exceeds 1e-10"
                ));
            }
            if reference.fluid_calls != solution.fluid_calls
                || reference.coupling_iterations != solution.coupling_iterations
                || reference.pressure_iterations != solution.pressure_iterations
            {
                return Err(format!("n={n} threads={threads}: iteration counts differ"));
            }
            for residual in [
                reference.divergence_rms,
                reference.momentum_rms,
                reference.force_law_rms,
                solution.divergence_rms,
                solution.momentum_rms,
                solution.force_law_rms,
            ] {
                if !residual.is_finite() || residual > 1e-8 {
                    return Err(format!(
                        "n={n} threads={threads}: residual {residual:.3e} exceeds 1e-8"
                    ));
                }
            }
            println!("{n},{threads},{du:.3e},{dp:.3e},{df:.3e},{ds:.3e},{bits},{:.3e},{:.3e},{:.3e},{:.3e},{:.3e},{:.3e},{},{},{},{},{},{}",
                reference.divergence_rms,solution.divergence_rms,
                reference.momentum_rms,solution.momentum_rms,
                reference.force_law_rms,solution.force_law_rms,
                reference.fluid_calls,solution.fluid_calls,
                reference.coupling_iterations,solution.coupling_iterations,
                reference.pressure_iterations,solution.pressure_iterations);
        }
    }
    Ok(())
}
