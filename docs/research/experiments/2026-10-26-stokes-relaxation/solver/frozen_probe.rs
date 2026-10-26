//! Static geometry experiment: recorded first-frame outlines, no Rapier step.
use std::error::Error;
use std::fs;
use stokes_relaxation_solver::fluid::{Marker, Solution, Stokes};

fn recorded_markers(path: &str, expected: usize) -> Result<Vec<Marker>, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let rows: Vec<Vec<&str>> = text
        .lines()
        .skip(1)
        .map(|s| s.split(',').collect())
        .collect();
    let first: Vec<_> = rows.iter().filter(|r| r[3] == "1").collect();
    let count: usize = first.first().ok_or("no frame-one outline")?[2].parse()?;
    if count != expected {
        return Err(format!("expected {expected} cells; got {count}").into());
    }
    let mut out = Vec::with_capacity(count * 4);
    for cell in 0..count {
        let mut nodes = [[0.0; 2]; 12];
        let mut seen = [false; 12];
        for row in first
            .iter()
            .filter(|r| r[5].parse::<usize>().ok() == Some(cell))
        {
            let index: usize = row[7].parse()?;
            nodes[index] = [row[8].parse()?, row[9].parse()?];
            seen[index] = true;
        }
        if !seen.iter().all(|v| *v) {
            return Err(format!("cell {cell}: missing node").into());
        }
        let center = [
            nodes.iter().map(|p| p[0]).sum::<f64>() / 12.0,
            nodes.iter().map(|p| p[1]).sum::<f64>() / 12.0,
        ];
        for index in [0, 3, 6, 9] {
            out.push(Marker {
                position: [
                    (nodes[index][0] + center[0]) * 0.5,
                    (nodes[index][1] + center[1]) * 0.5,
                ],
                velocity: [0.0, 0.0],
                drag: 0.0625,
            });
        }
    }
    Ok(out)
}

fn seed(solver: &mut Stokes, kind: &str, marker_count: usize) -> Result<Solution, String> {
    let mut prior = match kind {
        "poiseuille" => solver.solve(&[], [1.0, 0.0])?,
        "local" => {
            let point = Marker {
                position: [0.515, 0.485],
                velocity: [0.1, 0.04],
                drag: 0.0625,
            };
            solver.solve(&[point], [0.0, 0.0])?
        }
        _ => return Err(format!("unknown seed {kind}")),
    };
    // The seed came from a separate fluid source. It is not an earlier cell
    // force; after removing that source, all recorded cell forces start at zero.
    prior.marker_forces = vec![[0.0, 0.0]; marker_count];
    Ok(prior)
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn check(s: &Solution, markers: &[Marker]) -> Result<(), String> {
    if s.marker_forces.len() != markers.len() || s.sampled_velocity.len() != markers.len() {
        return Err("marker output count mismatch".into());
    }
    let mut sum_sq = 0.0;
    let mut max = 0.0_f64;
    let mut power = 0.0;
    let mut force_abs = 0.0;
    for (i, m) in markers.iter().enumerate() {
        let f = s.marker_forces[i];
        let u = s.sampled_velocity[i];
        for c in 0..2 {
            let closure = f[c] / m.drag + u[c] - m.velocity[c];
            sum_sq += closure * closure;
            max = max.max(closure.abs());
            power += f[c] * (u[c] - m.velocity[c]);
            force_abs += f[c].abs();
        }
    }
    let rms = (sum_sq / (2 * markers.len()) as f64).sqrt();
    for (name, a, b) in [
        ("rms", rms, s.force_law_rms),
        ("max", max, s.force_law_max_component_abs),
        ("power", power, s.exchange_power),
    ] {
        if (a - b).abs() > 1e-10 * a.abs().max(b.abs()).max(1.0) {
            return Err(format!(
                "{name}: independent value {a:.3e} vs solution {b:.3e}"
            ));
        }
    }
    if s.fluid_calls != 1 {
        return Err(format!(
            "expected one full fluid solve, got {}",
            s.fluid_calls
        ));
    }
    if !s.grid_velocity.iter().all(|x| x.is_finite()) || !s.pressure.iter().all(|x| x.is_finite()) {
        return Err("non-finite field".into());
    }
    if s.grid_force_balance_abs > 1e-10 * (1.0 + force_abs) {
        return Err(format!(
            "grid/point force imbalance {:.3e}",
            s.grid_force_balance_abs
        ));
    }
    Ok(())
}

fn bitwise_equal(a: &Solution, b: &Solution) -> bool {
    a.grid_velocity
        .iter()
        .zip(&b.grid_velocity)
        .all(|(x, y)| x.to_bits() == y.to_bits())
        && a.pressure
            .iter()
            .zip(&b.pressure)
            .all(|(x, y)| x.to_bits() == y.to_bits())
        && a.marker_forces
            .iter()
            .zip(&b.marker_forces)
            .all(|(x, y)| x[0].to_bits() == y[0].to_bits() && x[1].to_bits() == y[1].to_bits())
}

fn run(
    markers: &[Marker],
    layout: &str,
    kind: &str,
    omega: f64,
    steps: usize,
) -> Result<(), Box<dyn Error>> {
    let mut solver = Stokes::with_threads(64, 4)?;
    let mut prior = seed(&mut solver, kind, markers.len())?;
    let mut old = if omega == 1.0 {
        Some(Stokes::with_threads(64, 4)?)
    } else {
        None
    };
    let mut old_prior = if let Some(old_solver) = old.as_mut() {
        Some(seed(old_solver, kind, markers.len())?)
    } else {
        None
    };
    let initial = norm(&prior.grid_velocity);
    println!(
        "{layout},{kind},{omega:.1},0,{initial:.12e},0,{:.12e},0,0,0,0,0,0,0,0,seed",
        prior.grid_velocity[..4096].iter().sum::<f64>() / 4096.0
    );
    for iteration in 1..=steps {
        let next = solver.solve_relaxed(markers, [0.0, 0.0], Some(&prior), omega)?;
        check(&next, markers)?;
        let equality =
            if let (Some(old_solver), Some(old_prev)) = (old.as_mut(), old_prior.as_ref()) {
                let old_next = old_solver.solve_lagged(markers, [0.0, 0.0], Some(old_prev))?;
                let same = bitwise_equal(&next, &old_next);
                old_prior = Some(old_next);
                if !same {
                    return Err(
                        format!("omega=1 mismatch {layout} {kind} iteration={iteration}").into(),
                    );
                }
                "true"
            } else {
                "na"
            };
        let next_norm = norm(&next.grid_velocity);
        println!("{layout},{kind},{omega:.1},{iteration},{next_norm:.12e},{:.12e},{:.12e},{:.12e},{:.12e},{:.12e},{:.12e},{:.12e},{:.12e},{},{},{}",
            next_norm/norm(&prior.grid_velocity),
            next.grid_velocity[..4096].iter().sum::<f64>()/4096.0,
            next.force_law_rms,next.force_law_max_component_abs,next.exchange_power,
            next.grid_force_balance_abs,next.divergence_rms,next.momentum_rms,
            next.fluid_calls,next.pressure_iterations,equality);
        prior = next;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 4 {
        return Err(
            "usage: frozen_probe <16-cell-outlines.csv> <64-cell-outlines.csv> <steps>".into(),
        );
    }
    let steps: usize = args[3].parse()?;
    let m16 = recorded_markers(&args[1], 16)?;
    let m64 = recorded_markers(&args[2], 64)?;
    // First call has no history: old force is zero, and omega=1 retains the
    // exact legacy lagged path. Reject a prior with the wrong marker count.
    let one = [Marker {
        position: [0.5, 0.5],
        velocity: [0.1, 0.02],
        drag: 0.0625,
    }];
    let mut first_solver = Stokes::with_threads(64, 0)?;
    let first = first_solver.solve_relaxed(&one, [0.0, 0.0], None, 0.5)?;
    check_first(&first, &one)?;
    let mut old_solver = Stokes::with_threads(64, 0)?;
    let first_one = first_solver.solve_relaxed(&one, [0.0, 0.0], None, 1.0)?;
    let first_old = old_solver.solve_lagged(&one, [0.0, 0.0], None)?;
    if !bitwise_equal(&first_one, &first_old) {
        return Err("omega=1 first call differs from lagged".into());
    }
    let bad_prior = old_solver.solve(&[], [0.0, 0.0])?;
    if first_solver
        .solve_relaxed(&one, [0.0, 0.0], Some(&bad_prior), 0.5)
        .is_ok()
    {
        return Err("wrong prior marker count accepted".into());
    }
    for omega in [-0.1, 0.0, 1.1, f64::NAN] {
        if first_solver
            .solve_relaxed(&one, [0.0, 0.0], None, omega)
            .is_ok()
        {
            return Err("invalid omega accepted".into());
        }
    }
    println!("layout,seed,omega,iteration,velocity_norm,norm_ratio,mean_x_velocity,force_law_rms,force_law_max_component_abs,exchange_power,grid_force_balance_abs,divergence_rms,momentum_rms,fluid_calls,pressure_iterations,omega1_bitwise_lagged");
    for (layout, markers) in [("16", m16.as_slice()), ("64", m64.as_slice())] {
        for omega in [1.0, 0.5, 0.3] {
            run(markers, layout, "poiseuille", omega, steps)?;
        }
    }
    for omega in [1.0, 0.5, 0.3] {
        run(&m64, "64", "local", omega, steps)?;
    }
    Ok(())
}

fn check_first(s: &Solution, markers: &[Marker]) -> Result<(), String> {
    if s.fluid_calls != 2 {
        return Err(format!(
            "first relaxed call should use 2 full solves; got {}",
            s.fluid_calls
        ));
    }
    for (m, f) in markers.iter().zip(&s.marker_forces) {
        for c in 0..2 {
            let expected = 0.5 * m.drag * m.velocity[c];
            if (f[c] - expected).abs() > 1e-15 {
                return Err("first-call zero-history force differs".into());
            }
        }
    }
    Ok(())
}
