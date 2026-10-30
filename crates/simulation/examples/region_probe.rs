//! Headless driving across the two local regions; uses real production stepping.
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
    let seconds: f64 = args.get(1).map_or(20.0, |s| s.parse().unwrap());
    let out = args
        .get(2)
        .expect("region_probe <seconds> <new-output-directory>");
    std::fs::create_dir(out).expect("output must be new");
    let init = std::time::Instant::now();
    let config = Config {
        white_cells: simulation::WhiteCellOptions {
            count: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let dt = config.dt;
    let mut sim = Simulation::new_local_world(config);
    println!("init_ms={:.3}", init.elapsed().as_secs_f64() * 1000.0);
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
    writeln!(rows,"frame,whole_ms,fluid_ms,robot_x,robot_y,target,min_x,min_y,switches,handoff_ms,velocity_delta,divergence,momentum,exchange,area_error,wall_overshoot").unwrap();
    for frame in 1..=(seconds / dt as f64).round() as usize {
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
        }
        assert!(area_error < 0.25 && overshoot < 0.15);
        assert!(fluid.divergence_rms.is_finite() && fluid.momentum_rms.is_finite());
        assert!(fluid.exchange_error < 1e-7);
        writeln!(rows,"{frame},{:.9},{:.9},{:.9},{:.9},{target},{},{},{},{:.9},{:.9},{:.17e},{:.17e},{:.17e},{:.9},{:.9}",
            state.solve_ms,fluid.solve_ms,state.robot[0],state.robot[1],fluid.bounds[0][0],fluid.bounds[0][1],
            fluid.region_switches,fluid.handoff_ms,fluid.handoff_velocity_delta,fluid.divergence_rms,
            fluid.momentum_rms,fluid.exchange_error,area_error,overshoot).unwrap();
    }
    rows.flush().unwrap();
    println!(
        "switches={switches} final_target={target} frames={}",
        sim.snapshot().steps
    );
    assert!(switches >= 2, "probe did not exercise a return handoff");
}
