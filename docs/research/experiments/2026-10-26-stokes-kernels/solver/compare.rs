use stokes_kernel_solver::fluid::{Marker, Solution, Stokes};
use stokes_relaxation_solver::fluid::{
    Marker as OldMarker, Solution as OldSolution, Stokes as OldStokes,
};

fn compare(old: &OldSolution, new: &Solution) -> (f64, f64, f64) {
    let max_delta = |a: &[f64], b: &[f64]| {
        assert_eq!(a.len(), b.len());
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max)
    };
    for residual in [
        old.divergence_rms,
        old.momentum_rms,
        new.divergence_rms,
        new.momentum_rms,
    ] {
        assert!(
            residual.is_finite() && residual < 1e-8,
            "equation residual {residual}"
        );
    }
    let old_force: Vec<_> = old.marker_forces.iter().flatten().copied().collect();
    let new_force: Vec<_> = new.marker_forces.iter().flatten().copied().collect();
    (
        max_delta(&old.grid_velocity, &new.grid_velocity),
        max_delta(&old.pressure, &new.pressure),
        max_delta(&old_force, &new_force),
    )
}

fn main() -> Result<(), String> {
    let mut markers = Vec::new();
    for row in 0..4 {
        for col in 0..4 {
            for offset in [[0.045, 0.0], [0.0, 0.045], [-0.045, 0.0], [0.0, -0.045]] {
                markers.push(Marker {
                    position: [
                        (col as f64 + 0.5) / 4.0 + offset[0],
                        (row as f64 + 0.5) / 4.0 + offset[1],
                    ],
                    velocity: [
                        0.08 + 0.003 * (row * 4 + col) as f64,
                        0.01 * ((col % 3) as f64 - 1.0),
                    ],
                    drag: 0.0625,
                });
            }
        }
    }
    println!("n,threads,mode,step,max_grid_delta,max_pressure_delta,max_force_delta,new_divergence_rms,new_momentum_rms,old_pressure_iterations,new_pressure_iterations");
    for n in [32, 64, 128] {
        for threads in [0, 4] {
            let mut old = OldStokes::with_threads(n, threads)?;
            let mut new = Stokes::with_threads(n, threads)?;
            let old_markers: Vec<_> = markers
                .iter()
                .map(|m| OldMarker {
                    position: m.position,
                    velocity: m.velocity,
                    drag: m.drag,
                })
                .collect();
            let old_s = old.solve(&old_markers, [0.4, -0.1])?;
            let new_s = new.solve(&markers, [0.4, -0.1])?;
            let (du, dp, df) = compare(&old_s, &new_s);
            assert!(
                du.max(dp).max(df) < 1e-10,
                "cold solve changed beyond roundoff"
            );
            println!("{n},{threads},self-consistent,0,{du:.12e},{dp:.12e},{df:.12e},{:.12e},{:.12e},{},{}",new_s.divergence_rms,new_s.momentum_rms,old_s.pressure_iterations,new_s.pressure_iterations);
            let (mut prior_old, mut prior_new) = (None, None);
            for step in 0..6 {
                let o = old.solve_relaxed(
                    &old_markers,
                    [0.4 + 0.001 * step as f64, -0.1],
                    prior_old.as_ref(),
                    0.5,
                )?;
                let s = new.solve_relaxed(
                    &markers,
                    [0.4 + 0.001 * step as f64, -0.1],
                    prior_new.as_ref(),
                    0.5,
                )?;
                let (du, dp, df) = compare(&o, &s);
                println!("{n},{threads},relaxed,{step},{du:.12e},{dp:.12e},{df:.12e},{:.12e},{:.12e},{},{}",s.divergence_rms,s.momentum_rms,o.pressure_iterations,s.pressure_iterations);
                prior_old = Some(o);
                prior_new = Some(s);
            }
        }
    }
    Ok(())
}
