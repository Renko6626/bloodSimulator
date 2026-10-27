//! Temporary headless comparison; never installed as a production example.
use simulation::{Config, MoveInput, Simulation};
use std::io::{BufWriter, Write};

fn area(points: &[[f32; 2]]) -> f64 {
    (0..points.len())
        .map(|i| {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            a[0] as f64 * b[1] as f64 - a[1] as f64 * b[0] as f64
        })
        .sum::<f64>()
        .abs()
        * 0.5
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let seconds: f64 = args[1].parse().unwrap();
    let out = &args[2];
    std::fs::create_dir(out).unwrap();
    let start = std::time::Instant::now();
    let config = Config::default();
    let dt = config.dt;
    let mut sim = Simulation::new_local_world(config);
    println!("init_ms={:.6}", start.elapsed().as_secs_f64() * 1000.0);
    let map = sim.world_map().unwrap().clone();
    let first = sim.snapshot();
    let initial_areas: Vec<_> = first.cells.iter().map(|p| area(p)).collect();
    let targets = [
        [-13.0, 0.0],
        [-10.0, -4.0],
        [-6.0, -8.0],
        [-10.0, -4.0],
        [-16.0, 0.0],
    ];
    let mut target = 0;
    let mut switches = 0;
    let mut rows = BufWriter::new(std::fs::File::create(format!("{out}/frames.csv")).unwrap());
    writeln!(rows,"frame,whole_ms,fluid_ms,robot_x,robot_y,target,min_x,min_y,switches,handoff_ms,velocity_delta,divergence,momentum,exchange,area_error,wall_overshoot,active_cells,visible_cells,full_visible_cells,robot_weight,camera_margin").unwrap();
    let mut cells = BufWriter::new(std::fs::File::create(format!("{out}/cells.csv")).unwrap());
    writeln!(cells, "frame,cell,x,y,area").unwrap();
    let mut samples = BufWriter::new(std::fs::File::create(format!("{out}/samples.csv")).unwrap());
    writeln!(samples, "frame,x,y,weight,u,v").unwrap();
    for frame in 0..=(seconds / dt as f64).round() as usize {
        if frame > 0 {
            let before = sim.snapshot();
            let mut delta = [
                targets[target][0] - before.robot[0],
                targets[target][1] - before.robot[1],
            ];
            if delta[0].hypot(delta[1]) < 0.6 {
                target = (target + 1) % targets.len();
                delta = [
                    targets[target][0] - before.robot[0],
                    targets[target][1] - before.robot[1],
                ];
            }
            let norm = delta[0].hypot(delta[1]).max(1.0);
            sim.tick(MoveInput {
                x: delta[0] / norm,
                y: delta[1] / norm,
            });
        }
        let state = sim.snapshot();
        let fluid = state.fluid.as_ref().unwrap();
        switches += fluid.region_switches;
        let mut area_error = 0.0_f64;
        let mut overshoot = 0.0_f32;
        for (i, points) in state.cells.iter().enumerate() {
            assert!(points.iter().flatten().all(|v| v.is_finite()));
            area_error = area_error.max((area(points) / initial_areas[i] - 1.0).abs());
            for p in points {
                overshoot =
                    overshoot.max(map.signed_distance(rapier2d::prelude::Vector::from_array(*p)));
            }
            if frame % 60 == 0 {
                let x = points.iter().map(|p| p[0] as f64).sum::<f64>() / points.len() as f64;
                let y = points.iter().map(|p| p[1] as f64).sum::<f64>() / points.len() as f64;
                writeln!(cells, "{frame},{i},{x:.12},{y:.12},{:.12}", area(points)).unwrap();
            }
        }
        assert!(state.robot.iter().all(|v| v.is_finite()));
        assert!(area_error < 0.25 && overshoot < 0.15);
        assert!(fluid.divergence_rms < 1e-8 && fluid.momentum_rms < 1e-8);
        assert!(fluid.exchange_error < 1e-7);
        if frame % 60 == 0 {
            for p in [
                [-20.0, 0.0],
                [-16.0, 0.0],
                [-12.0, 0.0],
                [-10.0, -4.0],
                [-6.0, -8.0],
                [-10.0, 4.0],
                [-6.0, 8.0],
            ] {
                let (weight, [u, v]) = sim.window_trial_sample(p);
                assert!(u.is_finite() && v.is_finite());
                writeln!(
                    samples,
                    "{frame},{},{},{weight:.9},{u:.12},{v:.12}",
                    p[0], p[1]
                )
                .unwrap();
            }
        }
        if frame == 0 {
            continue;
        }
        let (visible, full, robot_weight, camera_margin) = sim.window_trial_details();
        writeln!(rows,"{frame},{:.9},{:.9},{:.9},{:.9},{target},{},{},{},{:.9},{:.9},{:.17e},{:.17e},{:.17e},{area_error:.9},{overshoot:.9},{},{visible},{full},{robot_weight:.9},{camera_margin:.9}",
            state.solve_ms,fluid.solve_ms,state.robot[0],state.robot[1],fluid.bounds[0][0],fluid.bounds[0][1],
            fluid.region_switches,fluid.handoff_ms,fluid.handoff_velocity_delta,fluid.divergence_rms,
            fluid.momentum_rms,fluid.exchange_error,fluid.active_cells).unwrap();
    }
    rows.flush().unwrap();
    cells.flush().unwrap();
    samples.flush().unwrap();
    println!(
        "switches={switches} final_target={target} frames={}",
        sim.snapshot().steps
    );
    assert!(switches >= 2, "probe did not exercise a return handoff");
}
