#[path = "../shared/fluid.rs"]
mod fluid;

use fluid::{Fluid, Marker};
use std::{
    env,
    f64::consts::PI,
    fs::File,
    io::{BufWriter, Write},
    time::Instant,
};

fn markers(center_x: f64, speed: f64, gamma: f64) -> [Marker; 4] {
    let points = [(-0.075, 0.0), (0.075, 0.0), (0.0, -0.075), (0.0, 0.075)];
    points.map(|(x, y)| Marker {
        position: [center_x + x, 0.5 + y],
        velocity: [speed, 0.0],
        drag: gamma / 4.0,
    })
}

fn run(
    case: &str,
    gamma: f64,
    duration: f64,
    output: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut f = Fluid::new(32, 2.0, 0.0);
    let dt = f.dt();
    let steps = (duration / dt).ceil() as usize;
    let mut writer = BufWriter::new(File::create(output)?);
    writeln!(writer, "step,time,center_x,center_wrapped_x,marker_seam_distance,target_speed,max_fluid_speed,minrho,maxrho,mass_drift,fluid_impulse_x,body_impulse_x,exchange_error_x,exchange_error_y,kernel_weight_error,force_jump,input_speed_jump,grid_cross,center_seam_cross,marker_seam_cross")?;
    let mut prev_force = None;
    let mut prev_speed = None;
    let mut prev_cells: Option<[isize; 4]> = None;
    let mut prev_center_seam = None;
    let mut prev_marker_seam = None;
    let mut max_jump_cross: f64 = 0.0;
    let mut max_jump_non_cross: f64 = 0.0;
    let mut max_unforced_jump_cross: f64 = 0.0;
    let mut max_unforced_jump_non_cross: f64 = 0.0;
    let mut max_exchange_error: f64 = 0.0;
    let mut max_kernel_weight_error: f64 = 0.0;
    let mut crossing_count = 0;
    let mut seam_count = 0;
    let mut minrho: f64 = f64::INFINITY;
    let mut maxrho: f64 = f64::NEG_INFINITY;
    let mut maxspeed: f64 = 0.0;
    let mut max_mass_drift: f64 = 0.0;
    let start = Instant::now();
    for step in 0..steps {
        let t = step as f64 * dt;
        let (cx, speed) = match case {
            // Range 0.3..0.7, smooth reversals and several grid crossings.
            "smooth" => (0.5 + 0.2 * (PI * t).sin(), 0.2 * PI * (PI * t).cos()),
            "seam" => (0.55 + 0.1 * t, 0.1),
            "frozen" => (0.5, 0.1),
            _ => return Err(format!("unknown case: {case}").into()),
        };
        let ms = markers(cx, speed, gamma);
        let marker_seam_distance = ms
            .iter()
            .map(|m| {
                let wrapped = m.position[0].rem_euclid(1.0);
                wrapped.min(1.0 - wrapped)
            })
            .fold(f64::INFINITY, f64::min);
        let cells = ms.map(|m| (m.position[0] * 32.0 - 0.5).floor() as isize);
        let center_seam = cx.floor() as isize;
        let marker_seam = (cx + 0.075).floor() as isize;
        let grid_cross = prev_cells.is_some_and(|old| old != cells);
        let seam_cross = prev_center_seam.is_some_and(|old| old != center_seam);
        let marker_seam_cross = prev_marker_seam.is_some_and(|old| old != marker_seam);
        let ex = f.advance(&ms).map_err(|e| format!("step {step}: {e:?}"))?;
        let total_body_x: f64 = ex.marker_impulses.iter().map(|p| p[0]).sum();
        let total_force = -total_body_x / dt;
        let force_jump = prev_force.map_or(0.0, |v: f64| (total_force - v).abs());
        let input_jump = prev_speed.map_or(0.0, |v: f64| (speed - v).abs());
        // The commanded velocity contributes at most Γ |ΔV| to a force change.
        let unforced_jump = (force_jump - gamma * input_jump).max(0.0);
        if grid_cross {
            crossing_count += 1;
            max_jump_cross = max_jump_cross.max(force_jump);
            max_unforced_jump_cross = max_unforced_jump_cross.max(unforced_jump);
        } else {
            max_jump_non_cross = max_jump_non_cross.max(force_jump);
            max_unforced_jump_non_cross = max_unforced_jump_non_cross.max(unforced_jump);
        }
        if seam_cross {
            seam_count += 1;
        }
        prev_cells = Some(cells);
        prev_center_seam = Some(center_seam);
        prev_marker_seam = Some(marker_seam);
        prev_force = Some(total_force);
        prev_speed = Some(speed);
        max_exchange_error = max_exchange_error.max(ex.impulse_error[0].hypot(ex.impulse_error[1]));
        max_kernel_weight_error = max_kernel_weight_error.max(ex.max_kernel_weight_error);
        let d = f.diagnostics();
        minrho = minrho.min(d.min_rho);
        maxrho = maxrho.max(d.max_rho);
        maxspeed = maxspeed.max(d.max_speed);
        let mass_drift = d.total_mass - 1.0;
        max_mass_drift = max_mass_drift.max(mass_drift.abs());
        writeln!(writer, "{step},{t:.12e},{cx:.12e},{:.12e},{marker_seam_distance:.12e},{speed:.12e},{:.12e},{:.12e},{:.12e},{mass_drift:.12e},{:.12e},{total_body_x:.12e},{:.12e},{:.12e},{:.12e},{force_jump:.12e},{input_jump:.12e},{},{},{}",
            cx.rem_euclid(1.0),
            d.max_speed,d.min_rho,d.max_rho,ex.fluid_impulse[0],ex.impulse_error[0],ex.impulse_error[1],ex.max_kernel_weight_error,
            grid_cross as u8,seam_cross as u8,marker_seam_cross as u8)?;
    }
    writer.flush()?;
    let field = f.field();
    let field_path = format!("{output}.field.csv");
    let mut field_writer = BufWriter::new(File::create(&field_path)?);
    writeln!(field_writer, "x,y,u,v,rho")?;
    for c in field {
        writeln!(
            field_writer,
            "{:.12e},{:.12e},{:.12e},{:.12e},{:.12e}",
            c.position[0], c.position[1], c.velocity[0], c.velocity[1], c.rho
        )?;
    }
    field_writer.flush()?;
    println!("case={case} gamma={gamma} steps={steps} elapsed_ms={:.3} minrho={minrho:.9} maxrho={maxrho:.9} maxspeed={maxspeed:.9} max_mass_drift={max_mass_drift:.3e} max_exchange_error={max_exchange_error:.3e} max_kernel_weight_error={max_kernel_weight_error:.3e} grid_crossings={crossing_count} seam_crossings={seam_count} max_force_jump_cross={max_jump_cross:.3e} max_force_jump_elsewhere={max_jump_non_cross:.3e} max_unforced_jump_cross={max_unforced_jump_cross:.3e} max_unforced_jump_elsewhere={max_unforced_jump_non_cross:.3e}",start.elapsed().as_secs_f64()*1e3);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        return Err("usage: moving CASE GAMMA DURATION OUTPUT.csv".into());
    }
    run(&args[1], args[2].parse()?, args[3].parse()?, &args[4])
}
