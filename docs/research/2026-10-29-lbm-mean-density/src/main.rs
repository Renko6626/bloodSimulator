#![allow(dead_code)]
mod lbm;
mod observe;
use blood_fluid_gpu::lbm as prod;
use lbm::*;
use observe::*;
use simulation::{
    circulation::{Flow, graph::GraphFlow},
    lbm_geometry::{opening_profiles, rasterize},
    map::{MapDefinition, VesselEdge},
};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};
fn main() {
    run().unwrap();
}
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let get = |key: &str, default: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| default.into())
    };
    let frames: usize = get("--frames", "1000")
        .parse()
        .map_err(|_| "invalid frames")?;
    let allow = args.iter().any(|a| a == "--allow-software");
    let out = PathBuf::from(get("--output", "results/paired"));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let nodes = vec![[-50., 24.], [24., 24.], [100., 62.], [-14., 100.]];
    let edges = [(0, 1), (1, 2), (1, 3)]
        .into_iter()
        .map(|(from, to)| VesselEdge {
            from,
            to,
            half_width: 3.,
            centerline: vec![nodes[from], nodes[to]],
        })
        .collect();
    let map = MapDefinition {
        nodes,
        edges,
        pump_edge: 0,
        spawn: [24., 24.],
        landmarks: vec![],
    };
    let flow = GraphFlow {
        pressures: vec![],
        rates: vec![18.529297, 18.529297 * 0.7, 18.529297 * 0.3],
        summary: Flow {
            pressure_drop: 0.,
            resistance: 0.,
            rate: 18.529297,
            mean_speed: 0.,
        },
    };
    let profile = GridProfile::Fine;
    let h = profile.cell_spacing();
    let dt = (1_f32 / 120.) / 7.;
    let n = profile.cells_per_side() as usize;
    let pspec = prod::WindowSpec {
        origin: prod::WindowOrigin {
            block_x: 0,
            block_y: 0,
        },
        profile: prod::GridProfile::Fine,
    };
    let raster = rasterize(&map, &flow, pspec)?;
    let geometry = WindowGeometry {
        solid: raster.geometry.solid.clone(),
        openings: raster
            .geometry
            .openings
            .iter()
            .map(|o| OpeningSpec {
                side: match o.side {
                    prod::WindowSide::Left => WindowSide::Left,
                    prod::WindowSide::Right => WindowSide::Right,
                    prod::WindowSide::Bottom => WindowSide::Bottom,
                    prod::WindowSide::Top => WindowSide::Top,
                },
                cells: o.cells.clone(),
                direction: o.direction,
                outward_flow: o.outward_flow,
                background_speed: o.background_speed,
            })
            .collect(),
    };
    let make = |density: f32| {
        LbmWindow::new(
            WindowSpec {
                origin: WindowOrigin {
                    block_x: 0,
                    block_y: 0,
                },
                profile,
            },
            WindowConfig {
                dt,
                viscosity: 0.05,
                allow_software: allow,
            },
            InitialField {
                density: vec![density; n * n],
                velocity: raster.initial.velocity.clone(),
            },
            geometry.clone(),
        )
    };
    let file = |name: &str| -> Result<BufWriter<File>, String> {
        Ok(BufWriter::new(
            File::create(out.join(name)).map_err(|e| e.to_string())?,
        ))
    };
    // One clearly nonzero Guo moment-only check, with2% mean offset; no fluid advance.
    {
        let w = make(1.02)?;
        let force = [0.02_f32, -0.01_f32];
        w.diagnostic_force_lattice(force);
        let force64 = force.map(f64::from);
        let a = w.diagnostic_populations();
        let fa = observe(&a, &geometry.solid, force64)?;
        let delta = (fa.mean - 1.) as f32;
        for c in fa.cells.iter().zip(&geometry.solid) {
            if !*c.1 && c.0.rho - f64::from(delta) <= 0. {
                return Err("nonpositive intended forced rho".into());
            }
        }
        w.remove_mean_offset(delta)?;
        let b = w.diagnostic_populations();
        let fb = observe(&b, &geometry.solid, force64)?;
        let (d, u, s, pass) = correction_limits(&fa, &fb, &geometry.solid, delta);
        let mut log = file("force-check.csv")?;
        writeln!(log,"mean_before,delta_f32,force_x_lattice,force_y_lattice,mean_after,density_shift_residual,velocity_change,guo_corrected_neqstress_change,passed").unwrap();
        writeln!(
            log,
            "{:.17e},{delta:.17e},{:.17e},{:.17e},{:.17e},{d:.17e},{u:.17e},{s:.17e},{pass}",
            fa.mean, force[0], force[1], fb.mean
        )
        .unwrap();
        log.flush().map_err(|e| e.to_string())?;
        println!(
            "force_moment_check mean_before={} delta={delta} force={force:?} density_residual={d:.3e} velocity_change={u:.3e} stress_change={s:.3e} passed={pass}",
            fa.mean
        );
        if !pass {
            return Err("forced correction moment limits failed".into());
        }
    }
    let mut baseline = make(1.)?;
    let mut controlled = make(1.)?;
    let bi = baseline.snapshot_diagnostics()?;
    let ci = controlled.snapshot_diagnostics()?;
    if bi.adapter_name != ci.adapter_name || bi.software_adapter != ci.software_adapter {
        return Err("paired windows must use same adapter".into());
    }
    let initial_b = baseline.diagnostic_populations();
    let initial_c = controlled.diagnostic_populations();
    if initial_b
        .iter()
        .zip(&initial_c)
        .any(|(a, b)| a.to_bits() != b.to_bits())
    {
        return Err("paired initial states differ".into());
    }
    let initial = observe(&initial_c, &geometry.solid, [0.; 2])?;
    let initial_mass = initial.count as f64 + initial.shifted;
    println!(
        "case=fork_a Fine frames={frames} paired=true same_initial_bits=true adapter={:?} software={} n={n} h={h} dt={dt} nu=0.05 tau={} fluid_cells={} initial_mass={initial_mass:.17e}",
        bi.adapter_name,
        bi.software_adapter,
        0.5 + 3. * 0.05 * dt / (h * h),
        initial.count
    );
    let mut base_out = file("baseline.csv")?;
    let mut control_out = file("controlled.csv")?;
    let mut corrections = file("corrections.csv")?;
    let mut compare = file("comparison.csv")?;
    let header = "frame,mean_rho,rho_min,rho_max,max_lattice,mass_change,mass_rel,velocity_error_world,volume_flux_error";
    writeln!(base_out, "{header}").unwrap();
    writeln!(
        control_out,
        "{header},solver_delta,removed_mass,cum_solver_delta,cum_removed_mass,budget_residual"
    )
    .unwrap();
    writeln!(corrections,"frame,delta_f64,delta_f32,mean_before,mean_after,actual_removed,expected_removed,removal_error,density_shift_residual,velocity_change,neqstress_component_change,mean_residual,passed").unwrap();
    writeln!(compare,"frame,region,velocity_rel_l2,velocity_abs_rms_lattice,velocity_abs_max_lattice,demeaned_rho_rel_l2,demeaned_rho_abs_rms,demeaned_rho_abs_max,neqstress_rel_l2,neqstress_abs_rms,neqstress_abs_max,guardrail_passed").unwrap();
    let (mut cumulative_solver, mut cumulative_removed, mut previous_shift) =
        (0., 0., initial.shifted);
    let mut guardrail_breaches = 0usize;
    for frame in 0..frames {
        let mut next = flow.clone();
        let pulse = 1. + 0.15 * (std::f32::consts::TAU * 1.2 * frame as f32 / 60.).sin();
        for q in &mut next.rates {
            *q *= pulse;
        }
        let (targets, profiles) = opening_profiles(&map, &next, &raster, pspec)?;
        baseline.set_opening_profiles(&targets, &profiles)?;
        controlled.set_opening_profiles(&targets, &profiles)?;
        for substep in 0..14 {
            for window in [&mut baseline, &mut controlled] {
                window.advance(StepInput {
                    acceleration: [0.; 2],
                })?;
                let field = window.field()?;
                for (i, p) in field.iter().enumerate() {
                    if geometry.solid[i] {
                        continue;
                    }
                    if p[0] <= 0. || !p[..3].iter().all(|v| v.is_finite()) {
                        return Err(format!("BAD frame={frame} substep={substep} cell={i}"));
                    }
                }
            }
        }
        let b = baseline.diagnostic_populations();
        let pre = controlled.diagnostic_populations();
        let fb = observe(&b, &geometry.solid, [0.; 2])?;
        let fp = observe(&pre, &geometry.solid, [0.; 2])?;
        let delta64 = fp.mean - 1.;
        let delta = delta64 as f32;
        for (i, c) in fp.cells.iter().enumerate() {
            if !geometry.solid[i] && c.rho - f64::from(delta) <= 0. {
                return Err(format!("nonpositive intended rho frame={frame} cell={i}"));
            }
        }
        controlled.remove_mean_offset(delta)?;
        let post = controlled.diagnostic_populations();
        let fc = observe(&post, &geometry.solid, [0.; 2])?;
        let (d, u, s, pass) = correction_limits(&fp, &fc, &geometry.solid, delta);
        let removed = fp.shifted - fc.shifted;
        let expected = f64::from(delta) * fp.count as f64;
        let mean_residual = (fc.mean - 1.).abs();
        writeln!(corrections,"{frame},{delta64:.17e},{delta:.17e},{:.17e},{:.17e},{removed:.17e},{expected:.17e},{:.17e},{d:.17e},{u:.17e},{s:.17e},{mean_residual:.17e},{pass}",fp.mean,fc.mean,removed-expected).unwrap();
        if !pass {
            return Err(format!("correction moment limits failed frame={frame}"));
        }
        let solver = fp.shifted - previous_shift;
        cumulative_solver += solver;
        cumulative_removed += removed;
        previous_shift = fc.shifted;
        let mass_change = fc.shifted - initial.shifted;
        let residual = mass_change - cumulative_solver + cumulative_removed;
        let (bu, bq) = boundary_errors(
            &fb,
            &geometry,
            &profiles,
            &targets,
            f64::from(h),
            f64::from(dt),
        );
        let (cu, cq) = boundary_errors(
            &fc,
            &geometry,
            &profiles,
            &targets,
            f64::from(h),
            f64::from(dt),
        );
        let bchange = fb.shifted - initial.shifted;
        writeln!(
            base_out,
            "{frame},{:.17e},{:.17e},{:.17e},{:.17e},{bchange:.17e},{:.17e},{bu:.17e},{bq:.17e}",
            fb.mean,
            fb.lo,
            fb.hi,
            fb.umax,
            bchange / initial_mass
        )
        .unwrap();
        writeln!(control_out,"{frame},{:.17e},{:.17e},{:.17e},{:.17e},{mass_change:.17e},{:.17e},{cu:.17e},{cq:.17e},{solver:.17e},{removed:.17e},{cumulative_solver:.17e},{cumulative_removed:.17e},{residual:.17e}",fc.mean,fc.lo,fc.hi,fc.umax,mass_change/initial_mass).unwrap();
        let mut frame_guard = true;
        let mut max_rel = 0_f64;
        for central in [false, true] {
            let diff = difference(&fb, &fc, &geometry.solid, n, central);
            let good = diff.u_rel <= 0.05 && diff.rho_rel <= 0.05 && diff.stress_rel <= 0.10;
            frame_guard &= good;
            max_rel = max_rel.max(diff.u_rel);
            let region = if central { "central" } else { "global" };
            writeln!(compare,"{frame},{region},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{good}",diff.u_rel,diff.u_rms,diff.u_max,diff.rho_rel,diff.rho_rms,diff.rho_max,diff.stress_rel,diff.stress_rms,diff.stress_max).unwrap();
        }
        guardrail_breaches += usize::from(!frame_guard);
        if frame % 50 == 49 || frame + 1 == frames {
            println!(
                "frame={frame} baseline_mean={} controlled_mean={} removed_cumulative={cumulative_removed:.8e} budget={residual:.3e} instant_du={u:.3e} paired_velocity_rel_max={max_rel:.5e} guardrail_breaches={guardrail_breaches}",
                fb.mean, fc.mean
            );
        }
        if frame + 1 == frames {
            for (name, data) in [
                ("baseline-final.bin", &b),
                ("controlled-pre-final.bin", &pre),
                ("controlled-post-final.bin", &post),
            ] {
                std::fs::write(out.join(name), bytemuck::cast_slice(data))
                    .map_err(|e| e.to_string())?;
            }
            std::fs::write(out.join("snapshot-layout.txt"),format!("frames={frames}\nnx={n}\nny={n}\nlayout=q-major9xf32-little-endian-shifted-g\nforce_lattice=[0,0]\nh={h}\ndt={dt}\n")).map_err(|e|e.to_string())?;
        }
    }
    for f in [
        &mut base_out,
        &mut control_out,
        &mut corrections,
        &mut compare,
    ] {
        f.flush().map_err(|e| e.to_string())?;
    }
    println!("COMPLETE guardrail_breaches={guardrail_breaches}");
    Ok(())
}
