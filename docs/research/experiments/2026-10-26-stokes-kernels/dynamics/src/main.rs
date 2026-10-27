use rapier2d::prelude::*;
use std::error::Error;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;
use stokes_kernel_solver::fluid::{Marker, Solution, Stokes};

const FRAME_DT: f64 = 1.0 / 60.0;
const SUB_DT: f64 = FRAME_DT / 2.0;
const NODES: usize = 12;
const RADIUS: f32 = 0.045;
const PARTICLE_RADIUS: f32 = RADIUS * 0.1;
const SELECTED: [usize; 4] = [0, 3, 6, 9];
const POINT_DRAG: f64 = 0.25 / 4.0;
const DRIVE_FORCE: f64 = 0.18;
const DRIVE_UNTIL: f64 = 0.5;

struct Options {
    n: usize,
    count: usize,
    threads: usize,
    omega: f64,
    frames: usize,
    prediction: f32,
    rapier_threads: usize,
    outdir: PathBuf,
}
fn options() -> Result<Options, Box<dyn Error>> {
    let mut n = 64;
    let mut count = None;
    let mut threads = 4;
    let mut omega = None;
    let mut frames = 360;
    let mut prediction = 0.002;
    let mut rapier_threads = 4;
    let mut outdir = None;
    let mut args = std::env::args().skip(1);
    while let Some(key) = args.next() {
        let value = args.next().ok_or(format!("missing value for {key}"))?;
        match key.as_str() {
            "--n" => n = value.parse::<usize>()?,
            "--count" => count = Some(value.parse::<usize>()?),
            "--threads" => threads = value.parse::<usize>()?,
            "--omega" => omega = Some(value.parse::<f64>()?),
            "--frames" => frames = value.parse::<usize>()?,
            "--prediction" => prediction = value.parse::<f32>()?,
            "--rapier-threads" => rapier_threads = value.parse::<usize>()?,
            "--outdir" => outdir = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option {key}").into()),
        }
    }
    let o = Options {
        n,
        count: count.ok_or("--count required")?,
        threads,
        omega: omega.ok_or("--omega required")?,
        frames,
        prediction,
        rapier_threads,
        outdir: outdir.ok_or("--outdir required")?,
    };
    if ![16, 64].contains(&o.count) || o.n != 64 || o.frames == 0 {
        return Err("count must be 16 or 64, n must be 64, frames positive".into());
    }
    if o.threads != 4 || !o.omega.is_finite() || !(0.0 < o.omega && o.omega <= 1.0) {
        return Err("threads must be 4 and omega must be in (0,1]".into());
    }
    if !o.prediction.is_finite() || o.prediction <= 0.0 || ![1, 2, 4].contains(&o.rapier_threads) {
        return Err(
            "prediction must be finite and positive; rapier-threads must be 1, 2 or 4".into(),
        );
    }
    Ok(o)
}
fn centroid(body: &SoftBody) -> Vector {
    body.particle_positions().sum::<Vector>() / NODES as f32
}
fn mean_velocity(body: &SoftBody) -> Vector {
    body.particle_velocities().sum::<Vector>() / NODES as f32
}
fn markers(body: &SoftBody) -> [Marker; 4] {
    let x = centroid(body);
    let v = mean_velocity(body);
    std::array::from_fn(|k| {
        let i = SELECTED[k];
        let p = (body.particle_position(i) + x) * 0.5;
        let w = (body.particle_velocity(i) + v) * 0.5;
        Marker {
            position: [p.x as f64, p.y as f64],
            velocity: [w.x as f64, w.y as f64],
            drag: POINT_DRAG,
        }
    })
}
fn signed_area(body: &SoftBody) -> f64 {
    let p: Vec<_> = body.particle_positions().collect();
    (0..NODES)
        .map(|i| p[i].perp_dot(p[(i + 1) % NODES]) as f64)
        .sum::<f64>()
        * 0.5
}
fn centers(count: usize) -> Vec<Vector> {
    if count == 64 {
        return (0..8)
            .flat_map(|row| {
                (0..8).map(move |col| {
                    Vector::new(
                        0.11 + 0.78 * col as f32 / 7.0,
                        0.11 + 0.78 * row as f32 / 7.0,
                    )
                })
            })
            .collect();
    }
    let cols = count / 4;
    (0..4)
        .flat_map(|row| {
            (0..cols).map(move |col| {
                let x = if cols == 4 {
                    0.2 + 0.2 * col as f32
                } else {
                    0.11 + 0.78 * col as f32 / 7.0
                };
                Vector::new(x, 0.17 + 0.22 * row as f32)
            })
        })
        .collect()
}
fn make_world(count: usize, prediction: f32) -> (PhysicsWorld, Vec<SoftBodyHandle>) {
    let mut world = PhysicsWorld::new();
    world.gravity = Vector::ZERO;
    world.integration_parameters.dt = SUB_DT as f32;
    // This research scene has diameter 0.09. Keep the contact lookahead explicit
    // instead of inheriting the 0.02 default used for meter-scale objects.
    world.integration_parameters.normalized_prediction_distance = prediction;
    let cells = centers(count)
        .into_iter()
        .map(|center| {
            world.insert_soft_body(
                SoftBodyBuilder::disk(center, RADIUS, NODES)
                    .mass(1.0)
                    .material(SoftBodyMaterial {
                        volume_softness: SpringCoefficients::new(30.0, 1.0),
                        ..SoftBodyMaterial::uniform(SpringCoefficients::new(12.0, 1.0))
                    })
                    .linear_damping(0.5)
                    .particle_radius(PARTICLE_RADIUS)
                    .surface_collider(ColliderBuilder::ball(PARTICLE_RADIUS).friction(0.12))
                    .can_sleep(false),
            )
        })
        .collect();
    (world, cells)
}
fn return_impulse(node_impulses: &mut [[Vector; NODES]], marker_index: usize, impulse: [f64; 2]) {
    let cell = marker_index / 4;
    let j = Vector::new(impulse[0] as f32, impulse[1] as f32);
    for node in &mut node_impulses[cell] {
        *node += j * (0.5 / NODES as f32);
    }
    node_impulses[cell][SELECTED[marker_index % 4]] += j * 0.5;
}
fn write_field<W: Write>(
    out: &mut W,
    opts: &Options,
    frame: usize,
    solver: &Stokes,
    solution: &Solution,
) -> Result<(), Box<dyn Error>> {
    for j in 0..16 {
        for i in 0..16 {
            let p = [(i as f64 + 0.5) / 16.0, (j as f64 + 0.5) / 16.0];
            let v = solver.sample_at(solution, p);
            writeln!(
                out,
                "{:.6},{},{},{},{:.9},{:.9},{:.12},{:.12}",
                opts.omega, opts.n, opts.count, frame, p[0], p[1], v[0], v[1]
            )?;
        }
    }
    Ok(())
}
fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn run(o: Options) -> Result<(), Box<dyn Error>> {
    create_dir_all(&o.outdir)?;
    // Rapier uses Rayon's global pool; the fluid solver owns its existing pool.
    // The phases run sequentially, and both pools have an explicit thread limit.
    rayon::ThreadPoolBuilder::new()
        .num_threads(o.rapier_threads)
        .build_global()?;
    println!(
        "rapier_parallel={} rapier_threads={} prediction={}",
        cfg!(feature = "parallel"),
        rayon::current_num_threads(),
        o.prediction
    );
    let (mut world, cells) = make_world(o.count, o.prediction);
    let init_started = Instant::now();
    let mut stokes = Stokes::with_threads(o.n, o.threads)?;
    let solver_init_ms = ms(init_started);
    let profiling = std::env::var_os("STOKES_PROFILE").is_some();
    stokes.set_profiling(profiling);
    let mut detail = if profiling {
        let mut out = BufWriter::new(File::create(o.outdir.join("profile.csv"))?);
        writeln!(out, "frame,inverse_calls,inverse_ms,copy_ms,forward_fft_ms,modes_ms,inverse_fft_ms,pressure_stencil_ms,kernel_ms,coupled_ms,diagnostics_ms")?;
        Some(out)
    } else {
        None
    };
    let mut init = BufWriter::new(File::create(o.outdir.join("init.csv"))?);
    writeln!(
        init,
        "n,threads,omega,solver_init_ms,rapier_parallel,rapier_threads,prediction"
    )?;
    writeln!(
        init,
        "{},{},{:.6},{solver_init_ms:.9},{},{},{}",
        o.n,
        o.threads,
        o.omega,
        cfg!(feature = "parallel"),
        rayon::current_num_threads(),
        o.prediction
    )?;
    let mut solution: Option<Solution> = None;
    let initial: Vec<_> = cells
        .iter()
        .map(|&h| signed_area(&world.soft_bodies[h]))
        .collect();
    let initial_centers: Vec<_> = cells
        .iter()
        .map(|&h| centroid(&world.soft_bodies[h]))
        .collect();
    let mut prev_positions: Vec<Vec<Vector>> = cells
        .iter()
        .map(|&h| world.soft_bodies[h].particle_positions().collect())
        .collect();
    let mut frames = BufWriter::new(File::create(o.outdir.join("frames.csv"))?);
    let mut cell_rows = BufWriter::new(File::create(o.outdir.join("cells.csv"))?);
    let mut outlines = BufWriter::new(File::create(o.outdir.join("outlines.csv"))?);
    let mut fields = BufWriter::new(File::create(o.outdir.join("fields.csv"))?);
    writeln!(frames, "omega,n,count,frame,t,drive_force_x,drive_impulse_x,fluid_impulse_x,fluid_impulse_y,exchange_mismatch,max_area_rel_error,max_node_jump,non_driven_mean_vx,non_driven_max_abs_vx,driven_nondriven_min_gap,solver_contacts_sum,solver_contacts_max,max_fluid_speed,stokes_coupling_iterations,stokes_fluid_calls,stokes_pressure_iterations,stokes_divergence_rms,stokes_momentum_rms,stokes_force_law_rms,stokes_force_law_max_component_abs,exchange_power_mean,point_force_update_ms,fluid_ms,rapier_ms,diagnostic_ms,whole_ms")?;
    writeln!(cell_rows, "omega,n,count,frame,t,cell,driven,center_x,center_y,displacement_x,velocity_x,velocity_y,area_rel_error")?;
    writeln!(outlines, "omega,n,count,frame,t,cell,driven,node,x,y")?;
    writeln!(fields, "omega,n,count,frame,x,y,ux,uy")?;
    let cols = if o.count == 64 { 8 } else { o.count / 4 };
    let driven = |c: usize| {
        if o.count == 64 {
            (c / cols == 3 || c / cols == 4) && c % cols == 3
        } else {
            (c / cols == 1 || c / cols == 2) && c % cols == 1
        }
    };
    for frame in 0..o.frames {
        let whole_start = Instant::now();
        let mut fluid_ms = 0.0;
        let (mut kernel_ms, mut coupled_ms, mut solver_diagnostic_ms) = (0.0, 0.0, 0.0);
        let mut rapier_ms = 0.0;
        let mut fluid_impulse = [0.0f64; 2];
        let mut body_impulse = [0.0f64; 2];
        let mut drive_impulse = 0.0f64;
        let mut solver_contacts_sum = 0usize;
        let mut solver_contacts_max = 0usize;
        let mut coupling_iterations = 0usize;
        let mut fluid_calls = 0usize;
        let mut pressure_iterations = 0usize;
        let mut divergence_rms = 0.0;
        let mut momentum_rms = 0.0;
        let mut force_law_rms = 0.0;
        let mut force_law_max_component_abs = 0.0f64;
        let mut exchange_power = 0.0f64;
        let mut point_force_update_ms = 0.0f64;
        for sub in 0..2 {
            let sub_start_t = (2 * frame + sub) as f64 * SUB_DT;
            let frozen: Vec<_> = cells
                .iter()
                .flat_map(|&h| markers(&world.soft_bodies[h]))
                .collect();
            let mut node_impulses = vec![[Vector::ZERO; NODES]; cells.len()];
            let started = Instant::now();
            let s = stokes
                .solve_relaxed(&frozen, [0.0, 0.0], solution.as_ref(), o.omega)
                .map_err(|e| format!("frame {} sub {} relaxed Stokes: {e}", frame + 1, sub))?;
            for (m, f) in s.marker_forces.iter().enumerate() {
                let impulse = [-f[0] * SUB_DT, -f[1] * SUB_DT];
                return_impulse(&mut node_impulses, m, impulse);
                fluid_impulse[0] += f[0] * SUB_DT;
                fluid_impulse[1] += f[1] * SUB_DT;
            }
            coupling_iterations += s.coupling_iterations;
            fluid_calls += s.fluid_calls;
            pressure_iterations += s.pressure_iterations;
            divergence_rms = divergence_rms.max(s.divergence_rms);
            momentum_rms = momentum_rms.max(s.momentum_rms);
            force_law_rms = force_law_rms.max(s.force_law_rms);
            force_law_max_component_abs =
                force_law_max_component_abs.max(s.force_law_max_component_abs);
            exchange_power += 0.5 * s.exchange_power;
            point_force_update_ms += s.point_force_update_ms;
            kernel_ms += s.kernel_prepare_ms;
            coupled_ms += s.coupled_solve_ms;
            solver_diagnostic_ms += s.diagnostics_ms;
            solution = Some(s);
            fluid_ms += ms(started);
            for ns in &node_impulses {
                for j in ns {
                    body_impulse[0] += j.x as f64;
                    body_impulse[1] += j.y as f64;
                }
            }
            let started = Instant::now();
            let drive = if sub_start_t < DRIVE_UNTIL {
                DRIVE_FORCE
            } else {
                0.0
            };
            for (c, (&h, impulses)) in cells.iter().zip(node_impulses.into_iter()).enumerate() {
                for (i, impulse) in impulses.into_iter().enumerate() {
                    let drive_j = if driven(c) {
                        drive * SUB_DT / NODES as f64
                    } else {
                        0.0
                    };
                    world.soft_bodies[h].apply_particle_impulse(
                        i,
                        impulse + Vector::new(drive_j as f32, 0.0),
                        true,
                    );
                }
                if driven(c) {
                    drive_impulse += drive * SUB_DT;
                }
            }
            world.step();
            let contacts = world.physics_pipeline.counters.solver.ncontacts;
            solver_contacts_sum += contacts;
            solver_contacts_max = solver_contacts_max.max(contacts);
            rapier_ms += ms(started);
        }
        let diagnostic_start = Instant::now();
        let t = (frame + 1) as f64 * FRAME_DT;
        let mismatch =
            (body_impulse[0] + fluid_impulse[0]).hypot(body_impulse[1] + fluid_impulse[1]);
        if mismatch > 1e-6 {
            return Err(format!("frame {} exchange mismatch {mismatch}", frame + 1).into());
        }
        let mut max_area_error = 0.0f64;
        let mut max_node_jump = 0.0f64;
        let mut non_driven_mean_vx = 0.0f64;
        let mut non_driven_max_abs_vx = 0.0f64;
        let mut non_driven_count = 0usize;
        let mut min_gap = f64::INFINITY;
        for (c, &h) in cells.iter().enumerate() {
            let b = &world.soft_bodies[h];
            if !b.particle_positions().all(|p| p.is_finite())
                || !b.particle_velocities().all(|v| v.is_finite())
            {
                return Err(format!("frame {} cell {} nonfinite", frame + 1, c).into());
            }
            let area = signed_area(b);
            if !area.is_finite() || area * initial[c] <= 0.0 {
                return Err(format!("frame {} cell {} area flip", frame + 1, c).into());
            }
            let err = (area / initial[c] - 1.0).abs();
            max_area_error = max_area_error.max(err);
            let v = mean_velocity(b);
            if !driven(c) {
                non_driven_count += 1;
                non_driven_mean_vx += v.x as f64;
                non_driven_max_abs_vx = non_driven_max_abs_vx.max((v.x as f64).abs());
            }
            for (i, p) in b.particle_positions().enumerate() {
                max_node_jump = max_node_jump.max((p - prev_positions[c][i]).length() as f64);
                prev_positions[c][i] = p;
            }
        }
        non_driven_mean_vx /= non_driven_count as f64;
        for (a, &ha) in cells.iter().enumerate() {
            if !driven(a) {
                continue;
            }
            for (b, &hb) in cells.iter().enumerate() {
                if driven(b) {
                    continue;
                }
                for pa in world.soft_bodies[ha].particle_positions() {
                    for pb in world.soft_bodies[hb].particle_positions() {
                        min_gap =
                            min_gap.min((pa - pb).length() as f64 - 2.0 * PARTICLE_RADIUS as f64);
                    }
                }
            }
        }
        let grid = &solution.as_ref().unwrap().grid_velocity;
        let mut max_fluid_speed = 0.0f64;
        for j in 0..o.n {
            for i in 0..o.n {
                let ux = 0.5 * (grid[j * o.n + i] + grid[j * o.n + (i + 1) % o.n]);
                let vy0 = if j == 0 {
                    0.0
                } else {
                    grid[o.n * o.n + (j - 1) * o.n + i]
                };
                let vy1 = if j + 1 == o.n {
                    0.0
                } else {
                    grid[o.n * o.n + j * o.n + i]
                };
                max_fluid_speed = max_fluid_speed.max(ux.hypot(0.5 * (vy0 + vy1)));
            }
        }
        let diagnostic_ms = ms(diagnostic_start);
        let whole_ms = ms(whole_start);
        if let Some(detail) = detail.as_mut() {
            let p = stokes.take_profile();
            writeln!(
                detail,
                "{},{},{},{},{},{},{},{},{},{},{}",
                frame + 1,
                p.inverse_calls,
                p.velocity_inverse_ms,
                p.inverse_prepare_copy_ms,
                p.forward_fft_ms,
                p.mode_gather_solve_scatter_ms,
                p.inverse_fft_ms,
                p.pressure_stencil_ms,
                kernel_ms,
                coupled_ms,
                solver_diagnostic_ms
            )?;
        }
        writeln!(
            frames,
            "{}",
            [
                format!("{:.6}", o.omega),
                o.n.to_string(),
                o.count.to_string(),
                (frame + 1).to_string(),
                format!("{t:.9}"),
                format!("{:.9}", if t <= DRIVE_UNTIL { DRIVE_FORCE } else { 0.0 }),
                format!("{drive_impulse:.12}"),
                format!("{:.12}", fluid_impulse[0]),
                format!("{:.12}", fluid_impulse[1]),
                format!("{mismatch:.12}"),
                format!("{max_area_error:.9}"),
                format!("{max_node_jump:.9}"),
                format!("{non_driven_mean_vx:.12}"),
                format!("{non_driven_max_abs_vx:.12}"),
                format!("{min_gap:.9}"),
                solver_contacts_sum.to_string(),
                solver_contacts_max.to_string(),
                format!("{max_fluid_speed:.12}"),
                coupling_iterations.to_string(),
                fluid_calls.to_string(),
                pressure_iterations.to_string(),
                format!("{divergence_rms:.12}"),
                format!("{momentum_rms:.12}"),
                format!("{force_law_rms:.12}"),
                format!("{force_law_max_component_abs:.12}"),
                format!("{exchange_power:.12}"),
                format!("{point_force_update_ms:.6}"),
                format!("{fluid_ms:.6}"),
                format!("{rapier_ms:.6}"),
                format!("{diagnostic_ms:.6}"),
                format!("{whole_ms:.6}"),
            ]
            .join(",")
        )?;
        for (c, &h) in cells.iter().enumerate() {
            let b = &world.soft_bodies[h];
            let x = centroid(b);
            let v = mean_velocity(b);
            let area_err = (signed_area(b) / initial[c] - 1.0).abs();
            writeln!(
                cell_rows,
                "{:.6},{},{},{},{:.9},{},{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9}",
                o.omega,
                o.n,
                o.count,
                frame + 1,
                t,
                c,
                driven(c) as u8,
                x.x,
                x.y,
                x.x - initial_centers[c].x,
                v.x,
                v.y,
                area_err
            )?;
        }
        if [1, 15, 30, 60, 120, 360].contains(&(frame + 1)) || frame + 1 == o.frames {
            for (c, &h) in cells.iter().enumerate() {
                for i in 0..NODES {
                    let p = world.soft_bodies[h].particle_position(i);
                    writeln!(
                        outlines,
                        "{:.6},{},{},{},{:.9},{},{},{},{:.9},{:.9}",
                        o.omega,
                        o.n,
                        o.count,
                        frame + 1,
                        t,
                        c,
                        driven(c) as u8,
                        i,
                        p.x,
                        p.y
                    )?;
                }
            }
            write_field(
                &mut fields,
                &o,
                frame + 1,
                &stokes,
                solution.as_ref().unwrap(),
            )?;
        }
    }
    println!(
        "relaxed omega={} {}^2 {} cells {} frames complete",
        o.omega, o.n, o.count, o.frames
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    run(options()?)
}
