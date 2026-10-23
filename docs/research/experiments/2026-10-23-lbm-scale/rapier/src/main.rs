use rapier2d::prelude::*;
use std::error::Error;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;

#[path = "../../shared/fluid.rs"]
mod fluid;
use fluid::{Fluid, Marker};

const FRAME_DT: f64 = 1.0 / 60.0;
const SUB_DT: f64 = 1.0 / 120.0;
const NODES: usize = 12;
const GAMMA: f64 = 0.25;
const SELECTED: [usize; 4] = [0, 3, 6, 9];
const CENTROID_WEIGHT: f32 = 0.5 / NODES as f32;
const NODE_WEIGHT: f32 = 0.5;
#[derive(Clone, Copy)]
struct ReturnRoute {
    cell: usize,
    selected_node: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Baseline,
    Cached,
}
impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Cached => "cached",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scene {
    Relax,
    Reversal,
    Squeeze,
    Scale,
    Counterflow,
}
impl Scene {
    fn name(self) -> &'static str {
        match self {
            Self::Relax => "relax",
            Self::Reversal => "reversal",
            Self::Squeeze => "squeeze",
            Self::Scale => "scale",
            Self::Counterflow => "counterflow",
        }
    }
}

struct Options {
    outdir: PathBuf,
    mode: Mode,
    count: usize,
    scene: Scene,
    duration: f64,
}
fn options() -> Result<Options, Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let mut outdir = None;
    let mut mode = None;
    let mut count = None;
    let mut scene = None;
    let mut duration = None;
    while let Some(arg) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value after {arg}"))?;
        match arg.as_str() {
            "--outdir" => outdir = Some(PathBuf::from(value)),
            "--mode" => {
                mode = Some(match value.as_str() {
                    "baseline" => Mode::Baseline,
                    "cached" => Mode::Cached,
                    _ => return Err("mode must be baseline or cached".into()),
                })
            }
            "--count" => count = Some(value.parse::<usize>()?),
            "--scene" => {
                scene = Some(match value.as_str() {
                    "relax" => Scene::Relax,
                    "reversal" => Scene::Reversal,
                    "squeeze" => Scene::Squeeze,
                    "scale" => Scene::Scale,
                    "counterflow" => Scene::Counterflow,
                    _ => {
                        return Err(
                            "scene must be relax, reversal, squeeze, scale, or counterflow".into(),
                        )
                    }
                })
            }
            "--duration" => duration = Some(value.parse::<f64>()?),
            _ => return Err(format!("unknown argument {arg}").into()),
        }
    }
    let opts = Options {
        outdir: outdir.ok_or("--outdir required")?,
        mode: mode.ok_or("--mode required")?,
        count: count.ok_or("--count required")?,
        scene: scene.ok_or("--scene required")?,
        duration: duration.ok_or("--duration required")?,
    };
    if !opts.duration.is_finite()
        || opts.duration <= 0.0
        || ((opts.duration / FRAME_DT).round() * FRAME_DT - opts.duration).abs() > 1e-9
    {
        return Err("duration must be a positive whole number of 1/60 frames".into());
    }
    if opts.scene == Scene::Scale && ![1, 4, 16].contains(&opts.count) {
        return Err("scale count must be 1, 4, or 16".into());
    }
    if opts.scene == Scene::Counterflow && opts.count != 16 {
        return Err("counterflow requires count 16".into());
    }
    if !matches!(opts.scene, Scene::Scale | Scene::Counterflow) && opts.count != 1 {
        return Err("reference scenes require count 1".into());
    }
    Ok(opts)
}
fn centroid(body: &SoftBody) -> Vector {
    body.particle_positions().sum::<Vector>() / NODES as f32
}
fn markers(body: &SoftBody) -> [Marker; 4] {
    let x = centroid(body);
    let v = body.particle_velocities().sum::<Vector>() / NODES as f32;
    std::array::from_fn(|k| {
        let i = SELECTED[k];
        let p = (body.particle_position(i) + x) * 0.5;
        let w = (body.particle_velocity(i) + v) * 0.5;
        Marker {
            position: [p.x as f64, p.y as f64],
            velocity: [w.x as f64, w.y as f64],
            drag: GAMMA / 4.0,
        }
    })
}
fn signed_area(body: &SoftBody) -> f64 {
    let xs: Vec<_> = body.particle_positions().collect();
    (0..NODES)
        .map(|i| xs[i].perp_dot(xs[(i + 1) % NODES]) as f64)
        .sum::<f64>()
        * 0.5
}
fn finite(body: &SoftBody) -> bool {
    body.particle_positions().all(|p| p.is_finite())
        && body.particle_velocities().all(|v| v.is_finite())
}
fn plate_gap(t: f64) -> f32 {
    if t < 0.5 {
        0.25
    } else if t < 1.25 {
        (0.25 - 0.13 * (t - 0.5) / 0.75) as f32
    } else if t < 1.75 {
        0.12
    } else if t < 2.5 {
        (0.12 + 0.13 * (t - 1.75) / 0.75) as f32
    } else {
        0.25
    }
}
fn centers(count: usize) -> Vec<Vector> {
    match count {
        1 => vec![Vector::new(0.5, 0.5)],
        4 => [0.35, 0.65]
            .into_iter()
            .flat_map(|y| [0.35, 0.65].map(move |x| Vector::new(x, y)))
            .collect(),
        16 => [0.17, 0.39, 0.61, 0.83]
            .into_iter()
            .flat_map(|y| [0.17, 0.39, 0.61, 0.83].map(move |x| Vector::new(x, y)))
            .collect(),
        _ => unreachable!(),
    }
}
fn make_world(
    opts: &Options,
) -> (
    PhysicsWorld,
    Vec<SoftBodyHandle>,
    Option<(RigidBodyHandle, RigidBodyHandle)>,
) {
    let mut world = PhysicsWorld::new();
    world.gravity = Vector::ZERO;
    world.integration_parameters.dt = SUB_DT as f32;
    let radius = if matches!(opts.scene, Scene::Scale | Scene::Counterflow) {
        0.075
    } else {
        0.15
    };
    let particle_radius = radius * 0.1;
    let points = if matches!(opts.scene, Scene::Scale | Scene::Counterflow) {
        centers(opts.count)
    } else {
        vec![Vector::new(0.5, 0.5)]
    };
    let mut handles = Vec::with_capacity(points.len());
    for (cell_index, center) in points.into_iter().enumerate() {
        let handle = world.insert_soft_body(
            SoftBodyBuilder::disk(center, radius, NODES)
                .mass(1.0)
                .material(SoftBodyMaterial {
                    volume_softness: SpringCoefficients::new(30.0, 1.0),
                    ..SoftBodyMaterial::uniform(SpringCoefficients::new(12.0, 1.0))
                })
                .linear_damping(0.5)
                .particle_radius(particle_radius)
                .surface_collider(ColliderBuilder::ball(particle_radius).friction(0.12))
                .can_sleep(false),
        );
        if matches!(opts.scene, Scene::Relax | Scene::Scale | Scene::Counterflow) {
            let speed = match opts.scene {
                Scene::Relax => 0.1,
                Scene::Counterflow if cell_index % 4 == 1 || cell_index % 4 == 3 => -0.05,
                _ => 0.05,
            };
            for i in 0..NODES {
                world.soft_bodies[handle].set_particle_velocity(i, Vector::new(speed, 0.0));
            }
        }
        handles.push(handle);
    }
    let plates = if opts.scene == Scene::Squeeze {
        let top = world
            .insert(
                RigidBodyBuilder::kinematic_position_based().translation(Vector::new(0.5, 0.25)),
                ColliderBuilder::cuboid(0.4, 0.015),
            )
            .0;
        let bottom = world
            .insert(
                RigidBodyBuilder::kinematic_position_based().translation(Vector::new(0.5, 0.75)),
                ColliderBuilder::cuboid(0.4, 0.015),
            )
            .0;
        Some((top, bottom))
    } else {
        None
    };
    (world, handles, plates)
}
fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
fn run(opts: Options) -> Result<(), Box<dyn Error>> {
    create_dir_all(&opts.outdir)?;
    let (mut world, cells, plates) = make_world(&opts);
    let mut fluid = Fluid::new(32, 2.0, 0.0);
    let lb_dt = fluid.dt();
    let frames = (opts.duration / FRAME_DT).round() as usize;
    let initial: Vec<f64> = cells
        .iter()
        .map(|&h| signed_area(&world.soft_bodies[h]))
        .collect();
    let return_routes: Vec<ReturnRoute> = (0..cells.len())
        .flat_map(|cell| {
            SELECTED.map(|selected_node| ReturnRoute {
                cell,
                selected_node,
            })
        })
        .collect();
    let mut previous_positions: Vec<Vec<Vector>> = cells
        .iter()
        .map(|&h| world.soft_bodies[h].particle_positions().collect())
        .collect();
    let mut previous_velocity: Vec<Vector> = cells
        .iter()
        .map(|&h| world.soft_bodies[h].particle_velocities().sum::<Vector>() / NODES as f32)
        .collect();
    let mut frame_csv = BufWriter::new(File::create(opts.outdir.join("frames.csv"))?);
    let mut outline_csv = BufWriter::new(File::create(opts.outdir.join("outlines.csv"))?);
    writeln!(frame_csv, "scene,count,mode,frame,t,center_x,center_y,velocity_x,velocity_y,accel_x,accel_y,max_node_speed,max_node_jump,min_area,max_area_rel_error,min_rho,max_rho,max_fluid_speed,total_mass,fluid_steps,fluid_time_mismatch,exchange_impulse_mismatch,drive_force_x,plate_half_gap,solver_contacts_sum,solver_contacts_max,closest_intercell_node_distance,prepare_ms,fluid_ms,node_transfer_ms,rapier_ms,diagnostics_ms,whole_ms")?;
    writeln!(
        outline_csv,
        "scene,count,mode,frame,t,cell,node,x,y,plate_top_y,plate_bottom_y"
    )?;
    let mut lb_steps_total = 0u64;
    for frame in 0..frames {
        let whole_start = Instant::now();
        let mut prepare_ms = 0.0;
        let mut fluid_ms = 0.0;
        let mut node_transfer_ms = 0.0;
        let mut rapier_ms = 0.0;
        let mut diagnostic_ms = 0.0;
        let mut frame_lb_steps = 0u64;
        let mut frame_exchange_mismatch: f64 = 0.0;
        let mut frame_drive_impulse = 0.0;
        let mut solver_contacts_sum = 0usize;
        let mut solver_contacts_max = 0usize;
        for sub in 0..2 {
            let t = (2 * frame + sub + 1) as f64 * SUB_DT;
            let sub_start = t - SUB_DT;
            let target_steps = ((t / lb_dt) + 1e-10).floor() as u64;
            let steps = target_steps - lb_steps_total;
            let frozen: Vec<Marker> = cells
                .iter()
                .flat_map(|&h| markers(&world.soft_bodies[h]))
                .collect();
            let started = Instant::now();
            let prepared = if opts.mode == Mode::Cached {
                Some(
                    fluid
                        .prepare_markers(&frozen)
                        .map_err(|e| format!("frame {} sub {} prepare {:?}", frame + 1, sub, e))?,
                )
            } else {
                None
            };
            prepare_ms += ms(started);
            let mut node_impulses = vec![[Vector::ZERO; NODES]; cells.len()];
            let mut fluid_impulse = [0.0f64; 2];
            let mut exchanges = Vec::with_capacity(steps as usize);
            let started = Instant::now();
            for _ in 0..steps {
                let exchange = match &prepared {
                    Some(p) => fluid.advance_prepared(p),
                    None => fluid.advance(&frozen),
                }
                .map_err(|e| format!("frame {} sub {} fluid {:?}", frame + 1, sub, e))?;
                if exchange.max_kernel_weight_error > 1e-10
                    || exchange.impulse_error[0].hypot(exchange.impulse_error[1]) > 1e-10
                {
                    return Err(format!(
                        "frame {} sub {} stencil or grid exchange mismatch",
                        frame + 1,
                        sub
                    )
                    .into());
                }
                exchanges.push(exchange);
            }
            fluid_ms += ms(started);
            let started = Instant::now();
            for exchange in exchanges {
                for (route, impulse) in return_routes.iter().zip(exchange.marker_impulses.iter()) {
                    let j = Vector::new(impulse[0] as f32, impulse[1] as f32);
                    for item in &mut node_impulses[route.cell] {
                        *item += j * CENTROID_WEIGHT;
                    }
                    node_impulses[route.cell][route.selected_node] += j * NODE_WEIGHT;
                }
                fluid_impulse[0] += exchange.fluid_impulse[0];
                fluid_impulse[1] += exchange.fluid_impulse[1];
            }
            node_transfer_ms += ms(started);
            frame_lb_steps += steps;
            lb_steps_total = target_steps;
            let started = Instant::now();
            let body_impulse = node_impulses
                .iter()
                .flat_map(|ns| ns.iter())
                .copied()
                .sum::<Vector>();
            frame_exchange_mismatch = frame_exchange_mismatch.max(
                ((body_impulse.x as f64 + fluid_impulse[0]).powi(2)
                    + (body_impulse.y as f64 + fluid_impulse[1]).powi(2))
                .sqrt(),
            );
            if let Some((top, bottom)) = plates {
                let gap = plate_gap(t);
                world.bodies[top].set_next_kinematic_translation(Vector::new(0.5, 0.5 - gap));
                world.bodies[bottom].set_next_kinematic_translation(Vector::new(0.5, 0.5 + gap));
            }
            let drive = match opts.scene {
                Scene::Reversal if sub_start < 1.5 => 0.08,
                Scene::Reversal if sub_start < 3.0 => -0.08,
                _ => 0.0,
            };
            frame_drive_impulse += drive * SUB_DT;
            node_transfer_ms += ms(started);
            let started = Instant::now();
            for (&h, impulses) in cells.iter().zip(node_impulses.into_iter()) {
                for (i, impulse) in impulses.into_iter().enumerate() {
                    world.soft_bodies[h].apply_particle_impulse(
                        i,
                        impulse + Vector::new((drive * SUB_DT / NODES as f64) as f32, 0.0),
                        true,
                    );
                }
            }
            world.step();
            let contacts = world.physics_pipeline.counters.solver.ncontacts;
            solver_contacts_sum += contacts;
            solver_contacts_max = solver_contacts_max.max(contacts);
            rapier_ms += ms(started);
        }
        let started = Instant::now();
        let mut center = Vector::ZERO;
        let mut velocity = Vector::ZERO;
        let mut acceleration = Vector::ZERO;
        let mut max_node_speed: f64 = 0.0;
        let mut max_node_jump: f64 = 0.0;
        let mut min_area = f64::INFINITY;
        let mut max_area_error: f64 = 0.0;
        let mut closest_intercell = f64::INFINITY;
        for (c, &h) in cells.iter().enumerate() {
            let body = &world.soft_bodies[h];
            if !finite(body) {
                return Err(format!("frame {} cell {} nonfinite node", frame + 1, c).into());
            }
            let signed = signed_area(body);
            if !signed.is_finite() || signed * initial[c] <= 0.0 {
                return Err(
                    format!("frame {} cell {} area vanished or inverted", frame + 1, c).into(),
                );
            }
            min_area = min_area.min(signed.abs());
            max_area_error = max_area_error.max((signed.abs() / initial[c].abs() - 1.0).abs());
            let cv = body.particle_velocities().sum::<Vector>() / NODES as f32;
            center += centroid(body);
            velocity += cv;
            acceleration += (cv - previous_velocity[c]) / FRAME_DT as f32;
            previous_velocity[c] = cv;
            for (i, p) in body.particle_positions().enumerate() {
                max_node_jump = max_node_jump.max((p - previous_positions[c][i]).length() as f64);
                previous_positions[c][i] = p;
            }
            max_node_speed = max_node_speed.max(
                body.particle_velocities()
                    .map(|v| v.length() as f64)
                    .fold(0.0, f64::max),
            );
            for &other in &cells[c + 1..] {
                for p in body.particle_positions() {
                    for q in world.soft_bodies[other].particle_positions() {
                        closest_intercell = closest_intercell.min((p - q).length() as f64);
                    }
                }
            }
        }
        center /= cells.len() as f32;
        velocity /= cells.len() as f32;
        acceleration /= cells.len() as f32;
        let stats = fluid.diagnostics();
        if !stats.min_rho.is_finite()
            || !stats.max_rho.is_finite()
            || !stats.max_speed.is_finite()
            || !stats.total_mass.is_finite()
            || stats.min_rho <= 0.0
        {
            return Err(format!("frame {} unstable fluid", frame + 1).into());
        }
        let mismatch = lb_steps_total as f64 * lb_dt - (frame + 1) as f64 * FRAME_DT;
        if mismatch.abs() >= lb_dt + 1e-12 {
            return Err(format!("frame {} clock mismatch", frame + 1).into());
        }
        if frame_exchange_mismatch > 1e-6 {
            return Err(format!(
                "frame {} fluid/body impulse mismatch {}",
                frame + 1,
                frame_exchange_mismatch
            )
            .into());
        }
        diagnostic_ms += ms(started);
        let gap = if plates.is_some() {
            plate_gap((frame + 1) as f64 * FRAME_DT) as f64
        } else {
            -1.0
        };
        let whole_ms = ms(whole_start);
        writeln!(frame_csv, "{},{},{},{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{},{:.9},{:.12},{:.9},{:.9},{},{},{:.9},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
            opts.scene.name(), opts.count, opts.mode.name(), frame + 1, (frame + 1) as f64 * FRAME_DT,
            center.x, center.y, velocity.x, velocity.y, acceleration.x, acceleration.y, max_node_speed, max_node_jump,
            min_area, max_area_error, stats.min_rho, stats.max_rho, stats.max_speed, stats.total_mass,
            frame_lb_steps, mismatch, frame_exchange_mismatch, frame_drive_impulse / FRAME_DT, gap,
            solver_contacts_sum, solver_contacts_max, closest_intercell, prepare_ms, fluid_ms, node_transfer_ms, rapier_ms, diagnostic_ms, whole_ms)?;
        if (frame + 1) % 6 == 0 || frame + 1 == frames {
            for (c, &h) in cells.iter().enumerate() {
                for i in 0..NODES {
                    let p = world.soft_bodies[h].particle_position(i);
                    writeln!(
                        outline_csv,
                        "{},{},{},{},{:.9},{},{},{:.9},{:.9},{:.9},{:.9}",
                        opts.scene.name(),
                        opts.count,
                        opts.mode.name(),
                        frame + 1,
                        (frame + 1) as f64 * FRAME_DT,
                        c,
                        i,
                        p.x,
                        p.y,
                        if gap >= 0.0 { 0.5 - gap } else { -1.0 },
                        if gap >= 0.0 { 0.5 + gap } else { -1.0 }
                    )?;
                }
            }
        }
    }
    println!(
        "{} {} count={} frames={} final time={:.6}",
        opts.scene.name(),
        opts.mode.name(),
        opts.count,
        frames,
        frames as f64 * FRAME_DT
    );
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    run(options()?)
}
