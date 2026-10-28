#![allow(dead_code)]
mod lbm;
use blood_fluid_gpu::lbm as prod;
use lbm::*;
use simulation::{
    circulation::{Flow, graph::GraphFlow},
    lbm_geometry::{opening_profiles, rasterize},
    map::{MapDefinition, VesselEdge},
};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    time::Instant,
};
fn main() {
    run().unwrap();
}
fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.
}
fn check(field: &[[f32; 4]], mask: &[bool]) -> Result<(), String> {
    for (i, p) in field.iter().enumerate() {
        if !mask[i] && (p[0] <= 0. || !p[..3].iter().all(|v| v.is_finite())) {
            return Err(format!("invalid field cell={i}"));
        }
    }
    Ok(())
}
fn density_mean(g: &[f32], mask: &[bool]) -> Result<(f64, f64), String> {
    let n = mask.len();
    let (mut total, mut count, mut lo) = (0., 0usize, f64::INFINITY);
    for i in 0..n {
        if mask[i] {
            continue;
        }
        let mut dr = 0.;
        for q in 0..9 {
            dr += f64::from(g[q * n + i]);
        }
        let rho = 1. + dr;
        if rho <= 0. || !rho.is_finite() {
            return Err(format!("invalid raw density cell={i}"));
        }
        total += dr;
        count += 1;
        lo = lo.min(rho);
    }
    Ok((1. + total / count as f64, lo))
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
    let warmup: usize = get("--warmup", "100")
        .parse()
        .map_err(|_| "invalid warmup")?;
    let measured: usize = get("--frames", "500")
        .parse()
        .map_err(|_| "invalid frames")?;
    if measured == 0 {
        return Err("need measured frames".into());
    }
    let allow = args.iter().any(|a| a == "--allow-software");
    let out = PathBuf::from(get("--output", "results/bench"));
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
    let mut csv = BufWriter::new(File::create(out.join("frames.csv")).map_err(|e| e.to_string())?);
    writeln!(csv,"mode,sample,iteration,total_wall_ms,budget_60hz_percent,profile_ms,enqueue_first7_cpu_ms,field1_wall_ms,enqueue_second7_cpu_ms,raw_read_wall_ms,cpu_mean_ms,normalization_enqueue_cpu_ms,field2_wall_ms,field_validity_cpu_ms,lattice_steps,dispatches,submissions,upload_calls,upload_bytes,readback_calls,readback_bytes,upload_cpu_ms,encoding_cpu_ms,submission_cpu_ms,readback_wait_wall_ms,readback_decode_cpu_ms,mean_before,delta_f32").unwrap();
    for corrected in [false, true] {
        let mode = if corrected {
            "mean_controlled"
        } else {
            "uncorrected"
        };
        let mut w = LbmWindow::new(
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
                density: raster.initial.density.clone(),
                velocity: raster.initial.velocity.clone(),
            },
            geometry.clone(),
        )?;
        let info = w.snapshot_diagnostics()?;
        println!(
            "BENCH mode={mode} fixture=fork_a Fine n=192 lattice_steps_per_frame=14 warmup={warmup} measured={measured} h={h} dt={dt} nu=.05 adapter={:?} software={} synchronization=two_field_readbacks init_excluded=true",
            info.adapter_name, info.software_adapter
        );
        for iteration in 0..warmup + measured {
            w.reset_work_stats();
            let total_start = Instant::now();
            let t = Instant::now();
            let mut next = flow.clone();
            let pulse = 1. + 0.15 * (std::f32::consts::TAU * 1.2 * iteration as f32 / 60.).sin();
            for q in &mut next.rates {
                *q *= pulse;
            }
            let (targets, profiles) = opening_profiles(&map, &next, &raster, pspec)?;
            w.set_opening_profiles(&targets, &profiles)?;
            let profile_ms = ms(t);
            let t = Instant::now();
            for _ in 0..7 {
                w.advance(StepInput {
                    acceleration: [0.; 2],
                })?;
            }
            let e1 = ms(t);
            let t = Instant::now();
            let f1 = w.field()?;
            let r1 = ms(t);
            let t = Instant::now();
            check(&f1, &geometry.solid)?;
            let mut validity = ms(t);
            let t = Instant::now();
            for _ in 0..7 {
                w.advance(StepInput {
                    acceleration: [0.; 2],
                })?;
            }
            let e2 = ms(t);
            let (mut raw_ms, mut mean_ms, mut normalization_ms, mut mean, mut delta) =
                (0., 0., 0., f64::NAN, 0_f32);
            if corrected {
                let t = Instant::now();
                let raw = w.diagnostic_populations();
                raw_ms = ms(t);
                let t = Instant::now();
                let (m, lo) = density_mean(&raw, &geometry.solid)?;
                mean = m;
                delta = (m - 1.) as f32;
                if lo - f64::from(delta) <= 0. {
                    return Err(format!(
                        "nonpositive intended density iteration={iteration}"
                    ));
                }
                mean_ms = ms(t);
                let t = Instant::now();
                w.remove_mean_offset(delta)?;
                normalization_ms = ms(t);
            }
            let t = Instant::now();
            let f2 = w.field()?;
            let r2 = ms(t);
            let t = Instant::now();
            check(&f2, &geometry.solid)?;
            validity += ms(t);
            let total = ms(total_start);
            let stats = w.work_stats();
            if iteration >= warmup {
                let sample = iteration - warmup;
                writeln!(csv,"{mode},{sample},{iteration},{total:.9},{:.9},{profile_ms:.9},{e1:.9},{r1:.9},{e2:.9},{raw_ms:.9},{mean_ms:.9},{normalization_ms:.9},{r2:.9},{validity:.9},{},{},{},{},{},{},{},{:.9},{:.9},{:.9},{:.9},{:.9},{mean:.17e},{delta:.17e}",total/(1000./60.)*100.,stats.lattice_steps,stats.dispatches,stats.submissions,stats.upload_calls,stats.upload_bytes,stats.readback_calls,stats.readback_bytes,stats.upload_ms,stats.encoding_ms,stats.submission_ms,stats.readback_wait_ms,stats.readback_decode_ms).unwrap();
            }
        }
        csv.flush().map_err(|e| e.to_string())?;
        println!("BENCH_COMPLETE mode={mode}");
    }
    println!("COMPLETE");
    Ok(())
}
