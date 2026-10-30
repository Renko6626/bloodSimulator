use simulation::{LbmOptions, Simulation, lbm_validation as fixture, map::ChunkId};
use std::{
    fs::{File, create_dir_all},
    io::{BufWriter, Write},
};

pub fn run(args: &[String]) -> Result<(), String> {
    let get = |name: &str, default: &str| {
        args.iter()
            .position(|s| s == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| default.into())
    };
    let frames: usize = get("--frames", "600")
        .parse()
        .map_err(|_| "invalid frames")?;
    let software = args.iter().any(|s| s == "--allow-software");
    let functional = args.iter().any(|s| s == "--functional");
    if frames != 600 || software != functional {
        return Err(
            "policy replay requires exactly 600 frames; software requires explicit --functional"
                .into(),
        );
    }
    let output = get("--output", "lbm-policy-results");
    create_dir_all(&output).map_err(|e| e.to_string())?;
    let mut status = File::create(format!("{output}/status.txt")).map_err(|e| e.to_string())?;
    let config = fixture::config();
    let map = fixture::policy_map();
    let mut sim = Simulation::try_new_policy_lbm_world(
        config.clone(),
        LbmOptions {
            allow_software: software,
            ..Default::default()
        },
        map.clone(),
    )?;
    writeln!(status, "functional={functional}\nconfig={config:?}\nroute=60 hold(14,24); 180 toward(24,24); 180 toward(14,24); 180 toward(24,24)\nwhite_cells=normal policy constructor; normal gameplay toggle unchanged\nmechanical guards=existing finite/area/wall guard; population=Aggregate ledger\ninitial={:?}", sim.policy_fluid_status()).map_err(|e| e.to_string())?;
    let mut csv =
        BufWriter::new(File::create(format!("{output}/policy.csv")).map_err(|e| e.to_string())?);
    writeln!(csv, "frame,time,input_x,input_y,robot_x,robot_y,frame_start_chunk_x,frame_start_chunk_y,actual_chunk_x,actual_chunk_y,window_active,origin_x,origin_y,activation,marker_force,robot_speed,robot_speed_delta,local_background_speed_delta,local_weight,total_red,detailed_red,abstract_red,quantity_error,white_detailed,area_min,area_max,node_wall,robot_wall").map_err(|e|e.to_string())?;
    let mut previous = sim.snapshot();
    let mut previous_active = false;
    let mut creates = 0;
    let mut releases = 0;
    let mut speed_previous = 0_f32;
    let mut max_quantity_error = 0_f64;
    let result = (|| -> Result<(), String> {
        for frame in 0..frames {
            let start = ChunkId::at(previous.robot);
            let input = fixture::policy_movement(frame, previous.robot);
            sim.try_tick(input)
                .map_err(|e| format!("frame {frame}: {e}"))?;
            let snapshot = sim.snapshot();
            let policy = sim.policy_fluid_status().ok_or("missing policy status")?;
            let q = fixture::quality(&snapshot, &map, &config, &[]);
            let marker = sim.lbm_stats().map_or(0., |s| s.max_cell_marker_force);
            let sample = sim
                .sample_world(snapshot.robot)
                .ok_or("missing policy sample")?;
            let velocity_delta = (sample.local_velocity[0] - sample.background_velocity[0])
                .hypot(sample.local_velocity[1] - sample.background_velocity[1]);
            let speed = (snapshot.robot[0] - previous.robot[0])
                .hypot(snapshot.robot[1] - previous.robot[1])
                / config.dt;
            let p = snapshot
                .population
                .as_ref()
                .ok_or("missing Aggregate ledger")?;
            let quantity_error =
                (p.detailed_count as f64 + p.abstract_count - p.total_count as f64).abs();
            max_quantity_error = max_quantity_error.max(quantity_error);
            let (ox, oy) = snapshot
                .lbm
                .as_ref()
                .map_or((String::new(), String::new()), |l| {
                    (
                        l.origin_blocks[0].to_string(),
                        l.origin_blocks[1].to_string(),
                    )
                });
            writeln!(csv,"{frame},{},{},{},{},{},{},{},{},{},{},{ox},{oy},{},{marker},{speed},{},{velocity_delta},{},{},{},{},{quantity_error},{},{},{},{},{}",snapshot.time,input.x,input.y,snapshot.robot[0],snapshot.robot[1],start.x,start.y,policy.center.x,policy.center.y,policy.window_active,policy.activation,speed-speed_previous,sample.local_weight,p.total_count,p.detailed_count,p.abstract_count,snapshot.cell_status.iter().filter(|s|s.white).count(),q.area_ratio_min,q.area_ratio_max,q.node_wall_max,q.robot_wall).map_err(|e|e.to_string())?;
            fixture::guard(q).map_err(|e| format!("frame {frame}: {e}"))?;
            if quantity_error > 1e-6 {
                return Err(format!("frame {frame}: quantity ledger mismatch {p:?}"));
            }
            if let Some(error) = &snapshot.lbm_status {
                return Err(format!("frame {frame}: {error}"));
            }
            if policy.window_active != snapshot.lbm.is_some() {
                return Err(format!("frame {frame}: window status disagrees"));
            }
            let allowed = start == (ChunkId { x: 1, y: 1 });
            if policy.window_active != allowed {
                return Err(format!(
                    "frame {frame}: frame-start selection mismatch {start:?}"
                ));
            }
            if let Some(l) = &snapshot.lbm {
                let selected = ChunkId {
                    x: l.origin_blocks[0] + 1,
                    y: l.origin_blocks[1] + 1,
                };
                if selected != start
                    || selected
                        .neighborhood(1)
                        .iter()
                        .any(|c| !map.fluid.as_ref().unwrap().policy_at(*c).allow_lbm)
                {
                    return Err(format!("frame {frame}: unchecked window neighborhood"));
                }
                if l.software_adapter && !functional {
                    return Err("hardware run rejects software adapter".into());
                }
                let residual = l
                    .opening_residuals
                    .iter()
                    .map(|v| v.abs())
                    .fold(0_f32, f32::max);
                if residual > 1e-3 {
                    return Err(format!("frame {frame}: flux residual {residual}"));
                }
            } else if policy.activation != 0. || marker != 0. || sample.local_weight != 0. {
                return Err(format!(
                    "frame {frame}: inactive window retained local force/weight"
                ));
            }
            if previous_active != policy.window_active {
                if policy.window_active {
                    creates += 1;
                } else {
                    releases += 1;
                }
                let adapter = snapshot
                    .lbm
                    .as_ref()
                    .map(|l| (&l.adapter_name, l.software_adapter));
                let event = format!(
                    "event frame={frame} active={} robot={:?} actual_center={:?} frame_start={start:?} activation={} adapter={adapter:?}",
                    policy.window_active, snapshot.robot, policy.center, policy.activation
                );
                writeln!(status, "{event}").map_err(|e| e.to_string())?;
                eprintln!("{event}");
            }
            if frame % 60 == 0 {
                eprintln!(
                    "policy frame={frame} robot={:?} active={} activation={} marker={marker}",
                    snapshot.robot, policy.window_active, policy.activation
                );
                csv.flush().map_err(|e| e.to_string())?;
            }
            previous_active = policy.window_active;
            speed_previous = speed;
            previous = snapshot;
        }
        if creates != 2 || releases != 1 || !previous_active {
            return Err(format!(
                "actual route incomplete: creates={creates} releases={releases} final_active={previous_active}"
            ));
        }
        Ok(())
    })();
    csv.flush().map_err(|e| e.to_string())?;
    writeln!(status,"steps={} creates={creates} releases={releases} max_quantity_error={max_quantity_error} final_robot={:?} result={result:?}",sim.snapshot().steps,sim.snapshot().robot).map_err(|e|e.to_string())?;
    if result.is_ok() {
        writeln!(
            status,
            "COMPLETE functional lifecycle only; no hardware FPS or physical accuracy acceptance"
        )
        .map_err(|e| e.to_string())?;
    }
    result
}
