#[cfg(not(feature = "lbm-gpu"))]
fn main() {
    eprintln!("requires --features lbm-gpu");
    std::process::exit(1);
}
#[cfg(feature = "lbm-gpu")]
fn main() {
    if let Err(e) = run() {
        eprintln!("FAILED {e}");
        std::process::exit(1);
    }
}
#[cfg(feature = "lbm-gpu")]
fn run() -> Result<(), String> {
    use blood_fluid_gpu::lbm::WindowConfig;
    use simulation::{
        LbmOptions, Simulation, lbm_validation as fixture, streaming_window::StreamingWindow,
    };
    use std::{
        fs::{File, create_dir_all},
        io::{BufWriter, Write},
        time::Instant,
    };
    let args: Vec<_> = std::env::args().collect();
    let get = |name: &str, default: &str| {
        args.iter()
            .position(|s| s == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| default.into())
    };
    let frames: usize = get("--frames", "3600")
        .parse()
        .map_err(|_| "invalid frames")?;
    let warmup: usize = get("--warmup", "120")
        .parse()
        .map_err(|_| "invalid warmup")?;
    let software = args.iter().any(|s| s == "--allow-software");
    let functional = args.iter().any(|s| s == "--functional");
    if software != functional || frames == 0 {
        return Err("software requires explicit --functional; frames must be positive".into());
    }
    let case = get("--case", "both");
    if !["hold", "move", "both"].contains(&case.as_str()) {
        return Err("case must be hold/move/both".into());
    }
    let output = get("--output", "lbm-gameplay-results");
    create_dir_all(&output).map_err(|e| e.to_string())?;
    let local_cells = args.iter().any(|s| s == "--local-cells");
    let mut config = fixture::config();
    if local_cells {
        config.cell_mode = simulation::CellSimulationMode::Local(Default::default());
    }
    let map = fixture::map();
    let options = LbmOptions {
        allow_software: software,
        ..Default::default()
    };
    let mut status = File::create(format!("{output}/status.txt")).map_err(|e| e.to_string())?;
    writeln!(status,"config={config:?}\nroute=hold(24,24); move x10↔28 y24, 720-frame legs; local_cells={local_cells}\nmechanical guards=finite; area ratio 0.5..1.5; node signed wall <=0.3; robot center signed wall <=-0.55").map_err(|e|e.to_string())?;
    for moving in [false, true] {
        let name = if moving { "move" } else { "hold" };
        if case != "both" && case != name {
            continue;
        }
        let mut sim = Simulation::new_lbm_validation(config.clone(), options.clone())?;
        let initial = sim.snapshot();
        let initial_centers = fixture::cell_centers(&initial);
        let stats = sim.lbm_stats().unwrap();
        // Initial origin is checked by the actual simulation; this checks the other route origin.
        let flow = simulation::circulation::graph::GraphNetwork::new(&map, config.viscosity)
            .solve(config.pressure * 1.15);
        let preflight = StreamingWindow::new(
            &map,
            &flow,
            [10., 24.],
            options.profile,
            WindowConfig {
                dt: stats.lattice_dt,
                viscosity: config.viscosity,
                allow_software: software,
            },
        )?;
        drop(preflight);
        let initial_lbm = initial.lbm.as_ref().ok_or("missing LBM")?;
        if initial_lbm.software_adapter && !functional {
            return Err("hardware run rejects software adapter".into());
        }
        writeln!(status,"{name} adapter={} software={} initial_rates={:?} initial_quality={:?} preflight=origins(0,0),(-1,0)",initial_lbm.adapter_name,initial_lbm.software_adapter,initial.branch_flows,fixture::quality(&initial,&map,&config,&initial_centers)).map_err(|e|e.to_string())?;
        let mut out = BufWriter::new(
            File::create(format!("{output}/{name}.csv")).map_err(|e| e.to_string())?,
        );
        writeln!(out,"phase,frame,time,input_x,input_y,robot_x,robot_y,active_cells,nearby_cells8,soft_contacts,cell_speed_mean,cell_speed_max,cell_displacement_mean,cell_displacement_max,area_ratio_min,area_ratio_max,node_wall_max,robot_wall,finite,density_min,density_max,density_sum,targets,fluxes,residual_max,branch_flows,region_switches,origin_x,origin_y,overlap_error,marker_force,prepare_ms,coupling_ms,lbm_ms,rapier_ms,step_wall_ms,snapshot_ms,quality_ms,lattice_steps,dispatches,submissions,readback_calls,readback_bytes,readback_wait_ms,total_cells,detailed_cells,abstract_cells,spawned,retired,cell_exchange_ms").map_err(|e|e.to_string())?;
        let mut last_region_switches = initial_lbm.region_switches;
        let mut previous = initial;
        let mut previous_centers = initial_centers.clone();
        let mut min_x = previous.robot[0];
        let mut max_x = min_x;
        let result = (|| -> Result<(), String> {
            for frame in 0..warmup + frames {
                let input = fixture::movement(moving, frame, warmup, previous.robot);
                let start = Instant::now();
                let step = sim.try_tick(input);
                let wall_ms = start.elapsed().as_secs_f64() * 1000.;
                let start = Instant::now();
                let snapshot = sim.snapshot();
                let snapshot_ms = start.elapsed().as_secs_f64() * 1000.;
                let start = Instant::now();
                let q = fixture::quality(
                    &snapshot,
                    &map,
                    &config,
                    if local_cells { &[] } else { &initial_centers },
                );
                let centers = fixture::cell_centers(&snapshot);
                let speeds: Vec<f32> = centers
                    .iter()
                    .take(if local_cells { 0 } else { centers.len() })
                    .zip(&previous_centers)
                    .map(|(a, b)| ((a[0] - b[0]).hypot(a[1] - b[1])) / config.dt)
                    .collect();
                let tracked = |v: f32| {
                    if local_cells {
                        String::new()
                    } else {
                        v.to_string()
                    }
                };
                let measured = |v: f32| {
                    if snapshot.cells.is_empty() {
                        String::new()
                    } else {
                        v.to_string()
                    }
                };
                let speed_mean = tracked(speeds.iter().sum::<f32>() / speeds.len().max(1) as f32);
                let speed_max = tracked(speeds.into_iter().fold(0_f32, f32::max));
                let quality_ms = start.elapsed().as_secs_f64() * 1000.;
                min_x = min_x.min(snapshot.robot[0]);
                max_x = max_x.max(snapshot.robot[0]);
                let l = snapshot.lbm.as_ref().ok_or_else(|| {
                    if let Err(error) = &step {
                        format!(
                            "frame {frame}: {error}; missing LBM snapshot: {:?}",
                            snapshot.lbm_status
                        )
                    } else {
                        format!(
                            "frame {frame}: missing LBM snapshot: {:?}",
                            snapshot.lbm_status
                        )
                    }
                })?;
                last_region_switches = l.region_switches;
                let s = sim.lbm_stats().unwrap();
                let w = sim.lbm_work_stats().unwrap();
                let values = |v: &[f32]| {
                    v.iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("|")
                };
                let residual = l
                    .opening_residuals
                    .iter()
                    .map(|v| v.abs())
                    .fold(0_f32, f32::max);
                write!(out,"{},{frame},{},{},{},{},{},{},{},{},{speed_mean},{speed_max},{},{},{},{},{},{},{},{},{},{},{},{},{residual},{},{},{},{},{},{},{},{},{},{},{wall_ms},{snapshot_ms},{quality_ms},{},{},{},{},{},{}",if frame<warmup{"warmup"}else{"measure"},snapshot.time,input.x,input.y,snapshot.robot[0],snapshot.robot[1],s.active_cell_count,q.nearby_cells,snapshot.soft_contacts,tracked(q.cell_displacement_mean),tracked(q.cell_displacement_max),measured(q.area_ratio_min),measured(q.area_ratio_max),measured(q.node_wall_max),q.robot_wall,q.finite,l.density_min,l.density_max,l.mass,values(&l.opening_targets),values(&l.opening_fluxes),values(&snapshot.branch_flows),l.region_switches,l.origin_blocks[0],l.origin_blocks[1],l.overlap_error,s.max_cell_marker_force,s.prepare_ms,s.coupling_ms,s.lbm_ms,s.rapier_ms,w.lattice_steps,w.dispatches,w.submissions,w.readback_calls,w.readback_bytes,w.readback_wait_ms).map_err(|e|e.to_string())?;
                if let Some(p) = &snapshot.population {
                    writeln!(
                        out,
                        ",{},{},{},{},{},{}",
                        p.total_count,
                        p.detailed_count,
                        p.abstract_count,
                        p.spawned_this_frame,
                        p.retired_this_frame,
                        p.exchange_ms
                    )
                    .map_err(|e| e.to_string())?;
                    if (p.detailed_count as f64 + p.abstract_count - p.total_count as f64).abs()
                        > 1e-6
                    {
                        return Err(format!("frame {frame}: population quantity mismatch {p:?}"));
                    }
                } else {
                    writeln!(out, ",,,,,,").map_err(|e| e.to_string())?;
                }
                if let Err(e) = step {
                    return Err(format!("frame {frame}: {e}"));
                }
                fixture::guard(q).map_err(|e| format!("frame {frame}: {e}"))?;
                if let Some(e) = &snapshot.lbm_status {
                    return Err(format!("frame {frame}: {e}"));
                }
                if residual > 1e-3 {
                    return Err(format!("frame {frame}: flux residual {residual}"));
                }
                if frame % 120 == 0 {
                    out.flush().map_err(|e| e.to_string())?;
                    eprintln!(
                        "{name} frame={frame} robot={:?} active={} contacts={} rho=[{},{}]",
                        snapshot.robot,
                        s.active_cell_count,
                        snapshot.soft_contacts,
                        l.density_min,
                        l.density_max
                    );
                }
                previous = snapshot;
                previous_centers = centers;
            }
            if moving && frames >= 1440 && last_region_switches == 0 {
                return Err("route did not produce an actual migration".into());
            }
            Ok(())
        })();
        out.flush().map_err(|e| e.to_string())?;
        writeln!(status,"{name} completed_steps={} min_x={min_x} max_x={max_x} region_switches={} result={result:?}",sim.snapshot().steps,last_region_switches).map_err(|e|e.to_string())?;
        status.flush().map_err(|e| e.to_string())?;
        result?;
    }
    writeln!(status, "COMPLETE").map_err(|e| e.to_string())?;
    Ok(())
}
