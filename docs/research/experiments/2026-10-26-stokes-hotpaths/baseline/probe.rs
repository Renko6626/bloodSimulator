use baseline_stokes_solver::fluid::{Marker, Solution, Stokes};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

fn markers(cells: usize, velocity: [f64; 2]) -> Vec<Marker> {
    let columns = if cells == 16 { 4 } else { 8 };
    let rows = cells / columns;
    let mut out = Vec::with_capacity(4 * cells);
    for row in 0..rows {
        for column in 0..columns {
            let center = [
                (column as f64 + 0.5) / columns as f64,
                (row as f64 + 0.5) / rows as f64,
            ];
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

fn flux(n: usize, s: &Solution) -> f64 {
    s.grid_velocity[..n * n].iter().sum::<f64>() / (n * n) as f64
}

fn center_velocity(n: usize, s: &Solution, i: usize, j: usize) -> [f64; 2] {
    let u = |i: usize, j: usize| s.grid_velocity[j * n + i % n];
    let v = |i: usize, j: usize| {
        if j == 0 || j == n {
            0.0
        } else {
            s.grid_velocity[n * n + (j - 1) * n + i % n]
        }
    };
    [0.5 * (u(i, j) + u(i + 1, j)), 0.5 * (v(i, j) + v(i, j + 1))]
}

fn write_field(
    path: &Path,
    n: usize,
    base: &Solution,
    coupled: &Solution,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut f = BufWriter::new(File::create(path)?);
    writeln!(
        f,
        "x,y,baseline_u,baseline_v,coupled_u,coupled_v,delta_u,delta_v"
    )?;
    for j in 0..n {
        for i in 0..n {
            let p = [(i as f64 + 0.5) / n as f64, (j as f64 + 0.5) / n as f64];
            let b = center_velocity(n, base, i, j);
            let c = center_velocity(n, coupled, i, j);
            writeln!(
                f,
                "{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12}",
                p[0],
                p[1],
                b[0],
                b[1],
                c[0],
                c[1],
                c[0] - b[0],
                c[1] - b[1]
            )?;
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::args()
        .nth(1)
        .ok_or("usage: static_probe <output-directory>")?;
    std::fs::create_dir_all(&out_dir)?;
    let mut raw = BufWriter::new(File::create(Path::new(&out_dir).join("samples.csv"))?);
    let mut lagged_raw = BufWriter::new(File::create(
        Path::new(&out_dir).join("lagged_samples.csv"),
    )?);
    writeln!(lagged_raw,"scenario,n,cells,total_ms,coupled_solve_ms,diagnostics_ms,flux,exact_flux,force_x,exact_force_x,force_law_rms,divergence_rms,momentum_rms,grid_force_balance_abs,fluid_calls")?;
    writeln!(raw,"scenario,n,cells,run,construct_ms,kernel_prepare_ms,coupled_solve_ms,diagnostics_ms,total_ms,flux,baseline_flux,flux_delta,max_grid_delta,rms_grid_delta,mean_marker_speed,force_x,force_y,fluid_calls,coupling_iterations,pressure_iterations,divergence_rms,momentum_rms,force_law_rms,grid_force_balance_abs,min_kernel_weight_sum,poiseuille_max_error")?;
    for (n, cells) in [(32, 16), (64, 16), (64, 32), (128, 64)] {
        let runs = if n == 128 { 1 } else { 3 };
        for (name, body_velocity, background_force) in [
            ("stationary", [0.0, 0.0], [1.0, 0.0]),
            ("moving", [0.1, 0.0], [0.0, 0.0]),
        ] {
            let construct_start = Instant::now();
            let mut solver = Stokes::new(n);
            let construct_ms = construct_start.elapsed().as_secs_f64() * 1000.0;
            let points = markers(cells, body_velocity);
            let base = solver.solve(&[], background_force)?;
            let mut exact_flux = 0.0;
            let mut exact_force_x = 0.0;
            let mut poiseuille_max_error: f64 = 0.0;
            if name == "stationary" {
                for j in 0..n {
                    let y = (j as f64 + 0.5) / n as f64;
                    for i in 0..n {
                        poiseuille_max_error = poiseuille_max_error
                            .max((base.grid_velocity[j * n + i] - 0.5 * y * (1.0 - y)).abs());
                    }
                }
            }
            for run in 0..runs {
                let start = Instant::now();
                let s = solver.solve(&points, background_force)?;
                let total_ms = start.elapsed().as_secs_f64() * 1000.0;
                let (mut max_delta, mut sum_delta) = (0.0_f64, 0.0);
                for (a, b) in s.grid_velocity.iter().zip(&base.grid_velocity) {
                    let d = (a - b).abs();
                    max_delta = max_delta.max(d);
                    sum_delta += d * d;
                }
                let rms_delta = (sum_delta / s.grid_velocity.len() as f64).sqrt();
                let mean_marker_speed = s
                    .sampled_velocity
                    .iter()
                    .map(|u| u[0].hypot(u[1]))
                    .sum::<f64>()
                    / points.len() as f64;
                let force_x = s.marker_forces.iter().map(|f| f[0]).sum::<f64>();
                let force_y = s.marker_forces.iter().map(|f| f[1]).sum::<f64>();
                if run == 0 {
                    exact_flux = flux(n, &s);
                    exact_force_x = force_x;
                }
                writeln!(raw,"{name},{n},{cells},{run},{construct_ms:.9},{:.9},{:.9},{:.9},{total_ms:.9},{:.12},{:.12},{:.12},{max_delta:.12},{rms_delta:.12},{mean_marker_speed:.12},{force_x:.12},{force_y:.12},{},{},{},{:.3e},{:.3e},{:.3e},{:.3e},{:.12},{:.3e}",
                    s.kernel_prepare_ms,s.coupled_solve_ms,s.diagnostics_ms,flux(n,&s),flux(n,&base),flux(n,&s)-flux(n,&base),
                    s.fluid_calls,s.coupling_iterations,s.pressure_iterations,s.divergence_rms,s.momentum_rms,s.force_law_rms,s.grid_force_balance_abs,s.min_kernel_weight_sum,poiseuille_max_error)?;
                eprintln!(
                    "{name} n={n} cells={cells} run={run} solve={:.3}ms calls={} closure={:.3e}",
                    total_ms, s.fluid_calls, s.force_law_rms
                );
                if n == 64 && cells == 16 && run == 0 {
                    write_field(
                        &Path::new(&out_dir).join(format!("field_{name}_n64_c16.csv")),
                        n,
                        &base,
                        &s,
                    )?;
                }
            }
            if cells == 16 && n <= 64 {
                // A single finite-drag update starting from the unladen background.
                // This costs one complete Stokes solve because the background was prepared above.
                let start = Instant::now();
                let lagged = solver.solve_lagged(&points, background_force, Some(&base))?;
                let total_ms = start.elapsed().as_secs_f64() * 1000.0;
                let fx = lagged.marker_forces.iter().map(|f| f[0]).sum::<f64>();
                writeln!(lagged_raw,"{name},{n},{cells},{total_ms:.9},{:.9},{:.9},{:.12},{exact_flux:.12},{fx:.12},{exact_force_x:.12},{:.3e},{:.3e},{:.3e},{:.3e},{}",
                    lagged.coupled_solve_ms,lagged.diagnostics_ms,flux(n,&lagged),lagged.force_law_rms,
                    lagged.divergence_rms,lagged.momentum_rms,lagged.grid_force_balance_abs,lagged.fluid_calls)?;
            }
        }
    }
    // Match the archived dense-mobility blob exactly: n=32, four points on radius .075,
    // prescribed x velocity .1, total drag .25, zero background force.
    let reference_markers: Vec<_> = [[0.575, 0.5], [0.5, 0.575], [0.425, 0.5], [0.5, 0.425]]
        .into_iter()
        .map(|position| Marker {
            position,
            velocity: [0.1, 0.0],
            drag: 0.0625,
        })
        .collect();
    let mut reference_solver = Stokes::new(32);
    let reference = reference_solver.solve(&reference_markers, [0.0, 0.0])?;
    let mut f = BufWriter::new(File::create(
        Path::new(&out_dir).join("one_cell_reference.csv"),
    )?);
    writeln!(f,"force_x,force_y,flux,force_law_rms,divergence_rms,momentum_rms,fluid_calls,coupling_iterations")?;
    writeln!(
        f,
        "{:.15},{:.15},{:.15},{:.3e},{:.3e},{:.3e},{},{}",
        reference.marker_forces.iter().map(|f| f[0]).sum::<f64>(),
        reference.marker_forces.iter().map(|f| f[1]).sum::<f64>(),
        flux(32, &reference),
        reference.force_law_rms,
        reference.divergence_rms,
        reference.momentum_rms,
        reference.fluid_calls,
        reference.coupling_iterations
    )?;
    Ok(())
}
