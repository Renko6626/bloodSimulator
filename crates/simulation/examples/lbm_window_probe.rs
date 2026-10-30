#[cfg(not(feature = "lbm-gpu"))]
fn main() {
    eprintln!("requires --features lbm-gpu");
}

#[cfg(feature = "lbm-gpu")]
mod probe {
    use blood_fluid_gpu::lbm::{LocalSampleStatus, StepInput, WindowConfig};
    use simulation::{
        Config, GridProfile, LbmOptions, MoveInput, Simulation,
        circulation::graph::GraphNetwork,
        map::{MapDefinition, VesselEdge},
        streaming_window::StreamingWindow,
    };
    use std::{
        env,
        fs::File,
        io::{BufWriter, Write},
        time::Instant,
    };

    fn map(fork: bool, offset: f32) -> MapDefinition {
        let mut nodes = vec![[-50., 24.], [100., 24.], [100., 80.], [-50., 80.]];
        let mut pairs = vec![(0, 1), (1, 2), (2, 3), (3, 0)];
        if fork {
            nodes = vec![
                [-50., 24.],
                [32. + offset, 24.],
                [100., 24.],
                [100., 80.],
                [-50., 80.],
                [32. + offset, 80.],
            ];
            pairs = vec![(0, 1), (1, 2), (2, 3), (3, 5), (5, 4), (4, 0), (1, 5)];
        }
        let edges = pairs
            .into_iter()
            .map(|(from, to)| VesselEdge {
                from,
                to,
                half_width: 3.,
                centerline: vec![nodes[from], nodes[to]],
            })
            .collect();
        MapDefinition {
            fluid: None,
            nodes,
            edges,
            pump_edge: 0,
            spawn: [31.5, 24.],
            landmarks: vec![],
        }
    }
    fn p95(v: &mut [f64]) -> f64 {
        v.sort_by(f64::total_cmp);
        v[((v.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)]
    }
    fn values(v: &[f32]) -> String {
        v.iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>()
            .join("|")
    }
    pub fn run() -> Result<(), String> {
        let args: Vec<String> = env::args().collect();
        let functional = args.iter().any(|s| s == "--functional");
        let software = args.iter().any(|s| s == "--allow-software");
        if functional != software {
            return Err("--functional and --allow-software must be supplied together".into());
        }
        let get = |name: &str, default: &str| -> String {
            args.iter()
                .position(|s| s == name)
                .and_then(|i| args.get(i + 1))
                .cloned()
                .unwrap_or_else(|| default.into())
        };
        let frames: usize = get("--frames", if functional { "4" } else { "720" })
            .parse()
            .map_err(|_| "invalid frames")?;
        let warmup: usize = get("--warmup", if functional { "2" } else { "20" })
            .parse()
            .map_err(|_| "invalid warmup")?;
        if frames == 0 || (!functional && (frames < 100 || warmup == 0)) {
            return Err("hardware mode requires >=100 frames and warmup".into());
        }
        let path = get("--output", "lbm-window-frames.csv");
        let mut out = BufWriter::new(File::create(&path).map_err(|e| e.to_string())?);
        writeln!(out,"case,profile,cells,band,feature_offset,pressure,phase,frame,physical_time,robot_x,robot_y,min_x,min_y,max_x,max_y,active_cells,adapter,software,hardware_measurement,opening_targets,opening_fluxes,density_min,density_max,density_sum,region_switches,overlap_error,immediate_center_density,immediate_center_velocity,immediate_center_momentum,marker_force,lattice_dt,lattice_steps_per_substep,lattice_steps,dispatches,submissions,upload_calls,upload_bytes,readback_calls,readback_bytes,upload_ms,encoding_ms,submission_ms,readback_wait_ms,readback_decode_ms,prepare_ms,coupling_ms,lbm_ms,rapier_ms,step_ms").unwrap();
        let mut reference_out = BufWriter::new(
            File::create(format!("{path}.reference.csv")).map_err(|e| e.to_string())?,
        );
        writeln!(reference_out, "case,frame,physical_time,shifted,overlap_error,center_density_delta,center_velocity_delta,center_momentum_delta,sample_count").unwrap();
        let mut warmup_out = BufWriter::new(
            File::create(format!("{path}.migration.csv")).map_err(|e| e.to_string())?,
        );
        writeln!(warmup_out, "case,profile,cells,band,feature_offset,frame,physical_time,substep_time,entry,density_delta,velocity_delta,momentum_delta").unwrap();
        let mut status_out =
            BufWriter::new(File::create(format!("{path}.status.csv")).map_err(|e| e.to_string())?);
        writeln!(
            status_out,
            "case,profile,cells,band,feature_offset,status,completed_frames,error"
        )
        .unwrap();
        let mut cases = Vec::new();
        for fork in [false, true] {
            for profile in [GridProfile::Fine, GridProfile::Coarse] {
                for cells in [0, 72] {
                    cases.push((fork, profile, cells, 4., 0.));
                }
            }
        }
        cases.extend([
            (false, GridProfile::Coarse, 72, 2., 0.),
            (true, GridProfile::Coarse, 0, 4., 8.),
        ]);
        if args.iter().any(|s| s == "--diagnose-fork-fine") {
            cases.retain(|(fork, profile, cells, _, offset)| {
                *fork && *profile == GridProfile::Fine && *cells == 0 && *offset == 0.
            });
        }
        let mut failures = 0;
        for (fork, profile, cells, band, offset) in cases {
            let mut completed_frames = 0;
            let result = (|| -> Result<(), String> {
                let config = Config {
                    cell_count: cells,
                    ..Default::default()
                };
                let options = LbmOptions {
                    profile,
                    outer_band: band,
                    allow_software: software,
                    ..Default::default()
                };
                let fixture = map(fork, offset);
                let network = GraphNetwork::new(&fixture, config.viscosity);
                let mut sim =
                    Simulation::new_lbm_world_with_map(config.clone(), options, fixture.clone())?;
                let mut reference = if profile == GridProfile::Coarse && cells == 0 && offset == 0.
                {
                    let stats = sim.lbm_stats().unwrap();
                    let wc = WindowConfig {
                        dt: stats.lattice_dt,
                        viscosity: config.viscosity,
                        allow_software: software,
                    };
                    let flow = network.solve(config.pressure);
                    Some((
                        StreamingWindow::new(&fixture, &flow, fixture.spawn, profile, wc)?,
                        StreamingWindow::new(&fixture, &flow, fixture.spawn, profile, wc)?,
                    ))
                } else {
                    None
                };
                let mut timings = Vec::new();
                let mut coupling = Vec::new();
                let mut lbm = Vec::new();
                let mut rapier = Vec::new();
                let mut emitted = 0;
                let mut first_warmup_time = None;
                for frame in 0..warmup + frames + 24 {
                    let previous_robot = sim.snapshot().robot;
                    let start = Instant::now();
                    let phase = if frame < warmup {
                        "warmup"
                    } else if frame < warmup + frames {
                        "hold"
                    } else if frame < warmup + frames + 16 {
                        "move"
                    } else {
                        "post"
                    };
                    let input = if phase == "warmup" || phase == "hold" {
                        MoveInput {
                            x: ((fixture.spawn[0] - previous_robot[0]) * 4.).clamp(-1., 1.),
                            y: ((fixture.spawn[1] - previous_robot[1]) * 4.).clamp(-1., 1.),
                        }
                    } else if phase == "move" {
                        MoveInput { x: 1., y: 0. }
                    } else {
                        MoveInput::default()
                    };
                    sim.try_tick(input)
                        .map_err(|e| format!("frame={frame} phase={phase}: {e}"))?;
                    let step_ms = start.elapsed().as_secs_f64() * 1000.;
                    let snap = sim.snapshot();
                    if let Some(error) = &snap.lbm_status {
                        return Err(format!(
                            "case={} profile={:?} cells={} frame={} phase={} diagnostic: {}",
                            if fork { "fork" } else { "straight" },
                            profile,
                            cells,
                            frame,
                            phase,
                            error
                        ));
                    }
                    let l = snap.lbm.ok_or("missing LBM snapshot")?;
                    let s = sim.lbm_stats().ok_or("missing LBM stats")?;
                    let w = sim.lbm_work_stats().ok_or("missing work stats")?;
                    if snap.lbm_status.is_some() {
                        return Err(format!("diagnostics: {:?}", snap.lbm_status));
                    }
                    if !functional && l.software_adapter {
                        return Err("software adapter rejected in hardware mode".into());
                    }
                    if args.iter().any(|s| s == "--diagnose-fork-fine") {
                        let mut max_speed = 0_f32;
                        for y in 0..192 {
                            for x in 0..192 {
                                let p = [
                                    l.bounds[0][0] + (x as f32 + 0.5) * 0.25,
                                    l.bounds[0][1] + (y as f32 + 0.5) * 0.25,
                                ];
                                let sample = sim.sample_world(p).unwrap();
                                if sample.status == LocalSampleStatus::Valid {
                                    max_speed = max_speed.max(
                                        sample.local_velocity[0].hypot(sample.local_velocity[1]),
                                    );
                                }
                            }
                        }
                        let wall = fixture
                            .signed_distance(rapier2d::prelude::Vector::from_array(snap.robot));
                        eprintln!(
                            "diagnostic frame={frame} phase={phase} robot={:?} wall_signed_distance={wall} soft_contacts={} bound={} lattice_dt={} steps_per_substep={} sampled_max_speed={} density=[{},{}] targets={:?} fluxes={:?}",
                            snap.robot,
                            snap.soft_contacts,
                            LbmOptions::default().velocity_bound,
                            s.lattice_dt,
                            s.lattice_steps_per_substep,
                            max_speed,
                            l.density_min,
                            l.density_max,
                            l.opening_targets,
                            l.opening_fluxes
                        );
                    }
                    if (phase == "warmup" || phase == "hold") && l.region_switches != 0 {
                        return Err(format!(
                            "unexpected shift during controlled hold in {phase}: robot {:?}",
                            snap.robot
                        ));
                    }
                    let substep_dt = (config.dt / config.substeps as f32) as f64;
                    if first_warmup_time.is_none() && !l.warmup.is_empty() {
                        first_warmup_time =
                            Some(s.elapsed_fluid_time - (l.warmup.len() - 1) as f64 * substep_dt);
                    }
                    for index in emitted..l.warmup.len() {
                        let d = l.warmup[index];
                        let time = first_warmup_time.unwrap() + index as f64 * substep_dt;
                        writeln!(
                            warmup_out,
                            "{},{:?},{},{},{},{},{},{},{},{},{},{}",
                            if fork { "fork" } else { "straight" },
                            profile,
                            cells,
                            band,
                            offset,
                            frame,
                            s.elapsed_fluid_time,
                            time,
                            index,
                            d[0],
                            d[1],
                            d[2]
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    emitted = l.warmup.len();
                    if let Some((moving, fixed)) = &mut reference {
                        let pressure = config.pressure
                            * (1.
                                + 0.15
                                    * (std::f32::consts::TAU
                                        * config.pulse_hz
                                        * frame as f32
                                        * config.dt)
                                        .sin());
                        let flow = network.solve(pressure);
                        moving.begin_frame();
                        fixed.begin_frame();
                        let shifted = moving.maybe_shift(&fixture, &flow, previous_robot)?;
                        fixed.maybe_shift(&fixture, &flow, fixture.spawn)?;
                        let steps = s.lattice_steps_per_substep * config.substeps;
                        moving.advance_batch(
                            StepInput {
                                acceleration: [0., 0.],
                            },
                            steps,
                        )?;
                        fixed.advance_batch(
                            StepInput {
                                acceleration: [0., 0.],
                            },
                            steps,
                        )?;
                        let mut max = [0_f32; 3];
                        let mut samples = 0;
                        for y in 40..56 {
                            for x in 64..88 {
                                let point = [x as f32 * 0.5 + 0.25, y as f32 * 0.5 + 0.25];
                                let a = moving.sample(point)?;
                                let b = fixed.sample(point)?;
                                if a.status == LocalSampleStatus::Valid
                                    && b.status == LocalSampleStatus::Valid
                                {
                                    samples += 1;
                                    max[0] = max[0].max((a.density - b.density).abs());
                                    max[1] = max[1].max(
                                        (a.velocity[0] - b.velocity[0])
                                            .hypot(a.velocity[1] - b.velocity[1]),
                                    );
                                    max[2] = max[2].max(
                                        (a.density * a.velocity[0] - b.density * b.velocity[0])
                                            .hypot(
                                                a.density * a.velocity[1]
                                                    - b.density * b.velocity[1],
                                            ),
                                    );
                                }
                            }
                        }
                        writeln!(
                            reference_out,
                            "{},{},{},{},{},{},{},{},{}",
                            if fork { "fork" } else { "straight" },
                            frame,
                            s.elapsed_fluid_time,
                            shifted,
                            moving.diagnostics.overlap_error,
                            max[0],
                            max[1],
                            max[2],
                            samples
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    if phase == "hold" {
                        timings.push(step_ms);
                        coupling.push(s.coupling_ms);
                        lbm.push(s.lbm_ms);
                        rapier.push(s.rapier_ms);
                    }
                    writeln!(out,"{},{:?},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}", if fork {"fork"}else{"straight"},profile,cells,band,offset,config.pressure,phase,frame,s.elapsed_fluid_time,snap.robot[0],snap.robot[1],l.bounds[0][0],l.bounds[0][1],l.bounds[1][0],l.bounds[1][1],s.active_cell_count,l.adapter_name.replace(','," "),l.software_adapter,!functional && !l.software_adapter,values(&l.opening_targets),values(&l.opening_fluxes),l.density_min,l.density_max,l.mass,l.region_switches,l.overlap_error,l.center[0],l.center[1],l.center[2],s.max_cell_marker_force,s.lattice_dt,s.lattice_steps_per_substep,w.lattice_steps,w.dispatches,w.submissions,w.upload_calls,w.upload_bytes,w.readback_calls,w.readback_bytes,w.upload_ms,w.encoding_ms,w.submission_ms,w.readback_wait_ms,w.readback_decode_ms,s.prepare_ms,s.coupling_ms,s.lbm_ms,s.rapier_ms,step_ms).map_err(|e|e.to_string())?;
                    completed_frames += 1;
                }
                eprintln!(
                    "case={} profile={:?} cells={} band={} offset={} p95_step_ms={} p95_coupling_ms={} p95_lbm_wait_inclusive_ms={} p95_rapier_ms={} functional={}",
                    if fork { "fork" } else { "straight" },
                    profile,
                    cells,
                    band,
                    offset,
                    p95(&mut timings),
                    p95(&mut coupling),
                    p95(&mut lbm),
                    p95(&mut rapier),
                    functional
                );
                Ok(())
            })();
            let (status, message) = match &result {
                Ok(()) => ("complete", String::new()),
                Err(error) => ("failed", error.replace([',', '\n', '\r'], " ")),
            };
            writeln!(
                status_out,
                "{},{:?},{},{},{},{},{},{}",
                if fork { "fork" } else { "straight" },
                profile,
                cells,
                band,
                offset,
                status,
                completed_frames,
                message
            )
            .map_err(|e| e.to_string())?;
            status_out.flush().map_err(|e| e.to_string())?;
            if let Err(error) = result {
                failures += 1;
                eprintln!(
                    "FAILED case={} profile={:?} cells={} band={} offset={}: {}",
                    if fork { "fork" } else { "straight" },
                    profile,
                    cells,
                    band,
                    offset,
                    error
                );
            }
        }
        out.flush().map_err(|e| e.to_string())?;
        reference_out.flush().map_err(|e| e.to_string())?;
        warmup_out.flush().map_err(|e| e.to_string())?;
        status_out.flush().map_err(|e| e.to_string())?;
        if failures != 0 {
            return Err(format!("{failures} cases failed; output incomplete"));
        }
        Ok(())
    }
}
#[cfg(feature = "lbm-gpu")]
fn main() {
    if let Err(e) = probe::run() {
        eprintln!("{e}");
        std::process::exit(1)
    }
}
