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

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Relax,
    Reversal,
    Squeeze,
}

impl Scenario {
    fn name(self) -> &'static str {
        match self {
            Self::Relax => "relax",
            Self::Reversal => "reversal",
            Self::Squeeze => "squeeze",
        }
    }
}

fn centroid(body: &SoftBody) -> Vector {
    body.particle_positions().sum::<Vector>() / NODES as f32
}

fn markers(body: &SoftBody) -> [Marker; 4] {
    let x = centroid(body);
    let v = body.particle_velocities().sum::<Vector>() / NODES as f32;
    std::array::from_fn(|k| {
        let i = 3 * k;
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
    body.particle_positions().all(|x| x.is_finite())
        && body.particle_velocities().all(|v| v.is_finite())
}

fn write_outline(
    writer: &mut BufWriter<File>,
    scenario: Scenario,
    frame: usize,
    body: &SoftBody,
    plate_gap: f64,
) -> std::io::Result<()> {
    for i in 0..NODES {
        let p = body.particle_position(i);
        writeln!(
            writer,
            "{},{},{:.9},{},{:.9},{:.9},{:.9},{:.9}",
            scenario.name(),
            frame,
            frame as f64 * FRAME_DT,
            i,
            p.x,
            p.y,
            if plate_gap >= 0.0 {
                0.5 - plate_gap
            } else {
                -1.0
            },
            if plate_gap >= 0.0 {
                0.5 + plate_gap
            } else {
                -1.0
            }
        )?;
    }
    Ok(())
}

fn make_world(
    scenario: Scenario,
) -> (
    PhysicsWorld,
    SoftBodyHandle,
    Option<(RigidBodyHandle, RigidBodyHandle)>,
) {
    let mut world = PhysicsWorld::new();
    world.gravity = Vector::ZERO;
    world.integration_parameters.dt = SUB_DT as f32;
    let handle = world.insert_soft_body(
        SoftBodyBuilder::disk(Vector::new(0.5, 0.5), 0.15, NODES)
            .mass(1.0)
            .material(SoftBodyMaterial {
                volume_softness: SpringCoefficients::new(30.0, 1.0),
                ..SoftBodyMaterial::uniform(SpringCoefficients::new(12.0, 1.0))
            })
            .linear_damping(0.5)
            .particle_radius(0.015)
            .surface_collider(ColliderBuilder::ball(0.015).friction(0.12))
            .can_sleep(false),
    );
    if matches!(scenario, Scenario::Relax) {
        for i in 0..NODES {
            world.soft_bodies[handle].set_particle_velocity(i, Vector::new(0.1, 0.0));
        }
    }
    let plates = if matches!(scenario, Scenario::Squeeze) {
        let top = world
            .insert(
                RigidBodyBuilder::kinematic_position_based()
                    .translation(Vector::new(0.5, 0.5 - 0.25)),
                ColliderBuilder::cuboid(0.4, 0.015),
            )
            .0;
        let bottom = world
            .insert(
                RigidBodyBuilder::kinematic_position_based()
                    .translation(Vector::new(0.5, 0.5 + 0.25)),
                ColliderBuilder::cuboid(0.4, 0.015),
            )
            .0;
        Some((top, bottom))
    } else {
        None
    };
    (world, handle, plates)
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

fn run(scenario: Scenario, seconds: f64, out: &PathBuf) -> Result<(), Box<dyn Error>> {
    let (mut world, cell, plates) = make_world(scenario);
    let mut fluid = Fluid::new(32, 2.0, 0.0);
    let lb_dt = fluid.dt();
    let frames = (seconds / FRAME_DT).round() as usize;
    let initial_signed_area = signed_area(&world.soft_bodies[cell]);
    let initial_area = initial_signed_area.abs();
    let mut frame_csv = BufWriter::new(File::create(out.join(format!("{}.csv", scenario.name())))?);
    let mut outline_csv = BufWriter::new(File::create(
        out.join(format!("{}_outline.csv", scenario.name())),
    )?);
    let mut field_csv = BufWriter::new(File::create(
        out.join(format!("{}_field.csv", scenario.name())),
    )?);
    writeln!(frame_csv, "frame,t,center_x,center_y,velocity_x,velocity_y,accel_x,accel_y,max_node_speed,max_node_jump,area,area_rel_error,min_rho,max_rho,max_fluid_speed,fluid_steps,fluid_time_mismatch,exchange_impulse_mismatch,drive_force_x,plate_half_gap,fluid_ms,rapier_ms,whole_ms")?;
    writeln!(
        outline_csv,
        "scenario,frame,t,node,x,y,plate_top_y,plate_bottom_y"
    )?;
    writeln!(field_csv, "scenario,frame,x,y,velocity_x,velocity_y,rho")?;
    write_outline(
        &mut outline_csv,
        scenario,
        0,
        &world.soft_bodies[cell],
        if plates.is_some() { 0.25 } else { -1.0 },
    )?;
    let mut lb_steps_total: u64 = 0;
    let mut worst_mismatch: f64 = 0.0;
    let mut max_area_error: f64 = 0.0;
    let mut max_node_speed_seen: f64 = 0.0;
    let mut min_rho_seen = f64::INFINITY;
    let mut max_rho_seen = f64::NEG_INFINITY;
    let mut max_fluid_speed_seen: f64 = 0.0;
    let mut max_impulse_mismatch: f64 = 0.0;
    let mut max_accel_seen: f64 = 0.0;
    let mut max_node_jump_seen: f64 = 0.0;
    let mut previous_velocity = world.soft_bodies[cell]
        .particle_velocities()
        .sum::<Vector>()
        / NODES as f32;
    let mut previous_positions: Vec<Vector> =
        world.soft_bodies[cell].particle_positions().collect();

    for frame in 0..frames {
        let whole_started = Instant::now();
        let mut fluid_ms = 0.0;
        let mut rapier_ms = 0.0;
        let mut frame_lb_steps = 0;
        let mut frame_exchange_mismatch: f64 = 0.0;
        let mut frame_drive_impulse = 0.0;
        for sub in 0..2 {
            let t = (2 * frame + sub + 1) as f64 * SUB_DT;
            let sub_start = t - SUB_DT;
            // Integer target on one global clock; no repeated ceil(17.067).
            let target_steps = ((t / lb_dt) + 1e-10).floor() as u64;
            let steps = target_steps - lb_steps_total;
            let frozen = markers(&world.soft_bodies[cell]);
            let mut node_impulses = [Vector::ZERO; NODES];
            let mut fluid_impulse = [0.0f64; 2];
            let start = Instant::now();
            for _ in 0..steps {
                let exchange = fluid.advance(&frozen).map_err(|error| {
                    format!(
                        "{} frame {} sub {}: fluid {:?}",
                        scenario.name(),
                        frame + 1,
                        sub,
                        error
                    )
                })?;
                if exchange.max_kernel_weight_error > 1e-10
                    || exchange.impulse_error[0].hypot(exchange.impulse_error[1]) > 1e-10
                {
                    return Err(format!(
                        "{} frame {}: stencil or lattice exchange mismatch",
                        scenario.name(),
                        frame + 1
                    )
                    .into());
                }
                for (k, impulse) in exchange.marker_impulses.iter().enumerate() {
                    // Marker position and velocity are 0.5 selected node + 0.5 centroid.
                    // The transpose weights preserve force, torque and instantaneous power.
                    let j = Vector::new(impulse[0] as f32, impulse[1] as f32);
                    for item in &mut node_impulses {
                        *item += j * (0.5 / NODES as f32);
                    }
                    node_impulses[3 * k] += j * 0.5;
                }
                fluid_impulse[0] += exchange.fluid_impulse[0];
                fluid_impulse[1] += exchange.fluid_impulse[1];
            }
            fluid_ms += start.elapsed().as_secs_f64() * 1000.0;
            frame_lb_steps += steps;
            lb_steps_total = target_steps;
            let body_impulse = node_impulses.iter().copied().sum::<Vector>();
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
            let drive = match scenario {
                Scenario::Reversal if sub_start < 1.5 => 0.08,
                Scenario::Reversal if sub_start < 3.0 => -0.08,
                _ => 0.0,
            };
            frame_drive_impulse += drive * SUB_DT;
            let start = Instant::now();
            for (i, impulse) in node_impulses.into_iter().enumerate() {
                world.soft_bodies[cell].apply_particle_impulse(
                    i,
                    impulse + Vector::new((drive * SUB_DT / NODES as f64) as f32, 0.0),
                    true,
                );
            }
            world.step();
            rapier_ms += start.elapsed().as_secs_f64() * 1000.0;
        }
        let body = &world.soft_bodies[cell];
        if !finite(body) {
            return Err(format!("{} frame {}: nonfinite node", scenario.name(), frame).into());
        }
        let center = centroid(body);
        let velocity = body.particle_velocities().sum::<Vector>() / NODES as f32;
        let acceleration = (velocity - previous_velocity) / FRAME_DT as f32;
        previous_velocity = velocity;
        let max_node_jump = body
            .particle_positions()
            .zip(previous_positions.iter())
            .map(|(p, old)| (p - *old).length() as f64)
            .fold(0.0, f64::max);
        previous_positions = body.particle_positions().collect();
        let max_node_speed = body
            .particle_velocities()
            .map(|v| v.length() as f64)
            .fold(0.0, f64::max);
        let signed = signed_area(body);
        if !signed.is_finite() || signed * initial_signed_area <= 0.0 {
            return Err(format!(
                "{} frame {}: cell area vanished or inverted",
                scenario.name(),
                frame + 1
            )
            .into());
        }
        let a = signed.abs();
        let area_error = (a / initial_area - 1.0).abs();
        let stats = fluid.diagnostics();
        if !stats.min_rho.is_finite()
            || !stats.max_rho.is_finite()
            || !stats.max_speed.is_finite()
            || !stats.total_mass.is_finite()
            || stats.min_rho <= 0.0
        {
            return Err(format!(
                "{} frame {}: unstable fluid density/velocity",
                scenario.name(),
                frame
            )
            .into());
        }
        let mismatch = lb_steps_total as f64 * lb_dt - (frame + 1) as f64 * FRAME_DT;
        if mismatch.abs() >= lb_dt + 1e-12 {
            return Err(format!("{} frame {}: clock mismatch", scenario.name(), frame).into());
        }
        max_area_error = max_area_error.max(area_error);
        max_node_speed_seen = max_node_speed_seen.max(max_node_speed);
        min_rho_seen = min_rho_seen.min(stats.min_rho);
        max_rho_seen = max_rho_seen.max(stats.max_rho);
        max_fluid_speed_seen = max_fluid_speed_seen.max(stats.max_speed);
        max_accel_seen = max_accel_seen.max(acceleration.length() as f64);
        max_node_jump_seen = max_node_jump_seen.max(max_node_jump);
        max_impulse_mismatch = max_impulse_mismatch.max(frame_exchange_mismatch);
        worst_mismatch = worst_mismatch.max(mismatch.abs());
        let drive = frame_drive_impulse / FRAME_DT;
        let gap = if plates.is_some() {
            plate_gap((frame + 1) as f64 * FRAME_DT) as f64
        } else {
            -1.0
        };
        writeln!(frame_csv, "{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{},{:.9},{:.12},{:.9},{:.9},{:.6},{:.6},{:.6}",
            frame + 1, (frame + 1) as f64 * FRAME_DT, center.x, center.y, velocity.x, velocity.y,
            acceleration.x, acceleration.y, max_node_speed, max_node_jump, a, area_error,
            stats.min_rho, stats.max_rho, stats.max_speed,
            frame_lb_steps, mismatch, frame_exchange_mismatch, drive, gap, fluid_ms, rapier_ms,
            whole_started.elapsed().as_secs_f64() * 1000.0,
        )?;
        if (frame + 1) % 6 == 0 || frame + 1 == frames {
            write_outline(&mut outline_csv, scenario, frame + 1, body, gap)?;
        }
        if (frame + 1) % 30 == 0 || frame + 1 == frames {
            for point in fluid.field() {
                writeln!(
                    field_csv,
                    "{},{},{:.9},{:.9},{:.9},{:.9},{:.9}",
                    scenario.name(),
                    frame + 1,
                    point.position[0],
                    point.position[1],
                    point.velocity[0],
                    point.velocity[1],
                    point.rho
                )?;
            }
        }
    }
    println!("{} frames={} max_area_error={:.6} max_node_speed={:.6} max_center_accel={:.6} max_node_jump={:.6} rho=[{:.6},{:.6}] max_fluid_speed={:.6} clock_error={:.9} impulse_error={:.12}",
        scenario.name(), frames, max_area_error, max_node_speed_seen, max_accel_seen, max_node_jump_seen, min_rho_seen,
        max_rho_seen, max_fluid_speed_seen, worst_mismatch, max_impulse_mismatch);
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().unwrap_or_else(|| "results".to_owned()));
    let seconds: f64 = args.next().unwrap_or_else(|| "3".to_owned()).parse()?;
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err("seconds must be positive and finite".into());
    }
    create_dir_all(&out)?;
    for scene in [Scenario::Relax, Scenario::Reversal, Scenario::Squeeze] {
        run(scene, seconds, &out)?;
    }
    Ok(())
}
