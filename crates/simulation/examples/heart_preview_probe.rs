//! Actual bounded MoveInput traversal; no teleporting or synthetic geometry proof.
use rapier2d::prelude::Vector;
use simulation::{Config, MoveInput, Simulation, map::heart_preview_route};
fn main() {
    let mut sim = Simulation::new_heart_preview(Config {
        robot_speed: 8.,
        ..Default::default()
    });
    let mut peak = 0;
    let mut min_clearance = f32::INFINITY;
    let mut min_pose = [0.; 2];
    let mut min_segment = "";
    let mut entries = [0.; 2];
    println!("waypoint,x,y,wall_distance,loaded_chunks");
    for (name, target) in heart_preview_route() {
        let mut reached = false;
        for _ in 0..2400 {
            let state = sim.snapshot();
            let d = [target[0] - state.robot[0], target[1] - state.robot[1]];
            if d[0].hypot(d[1]) < 0.25 {
                reached = true;
                break;
            }
            sim.tick(MoveInput {
                x: d[0] / d[0].hypot(d[1]) * (d[0].hypot(d[1]) * 2.).min(1.),
                y: d[1] / d[0].hypot(d[1]) * (d[0].hypot(d[1]) * 2.).min(1.),
            });
            let state = sim.snapshot();
            let distance = sim
                .world_map()
                .unwrap()
                .signed_distance(Vector::new(state.robot[0], state.robot[1]));
            if -distance < min_clearance {
                min_clearance = -distance;
                min_pose = state.robot;
                min_segment = name;
            }
            assert!(state.cells.is_empty());
            assert_eq!(state.immunity.white_bodies, 0);
            assert!(state.branch_flows.iter().all(|v| *v == 0.));
            assert_eq!(state.flow.rate, 0.);
            assert_eq!(state.flow.pressure_drop, 0.);
            peak = peak.max(state.streaming.unwrap().loaded_chunks);
        }
        let s = sim.snapshot();
        assert!(reached, "failed {name}: {:?}, target {target:?}", s.robot);
        let wall = sim
            .world_map()
            .unwrap()
            .signed_distance(Vector::new(s.robot[0], s.robot[1]));
        println!(
            "{name},{:.3},{:.3},{wall:.3},{}",
            s.robot[0],
            s.robot[1],
            s.streaming.unwrap().loaded_chunks
        );
        match name {
            "right_entry" => entries[0] = s.robot[1],
            "left_entry" => entries[1] = s.robot[1],
            "right_deep" => {
                let span = s.robot[1] - entries[0];
                assert!(span >= 80.);
                println!("right_actual_span={span:.3}");
            }
            "left_deep" => {
                let span = s.robot[1] - entries[1];
                assert!(span >= 80.);
                println!("left_actual_span={span:.3}");
            }
            _ => {}
        }
    }
    println!("min_pose={min_pose:?},segment={min_segment}");
    assert!(
        min_clearance >= 1.0,
        "insufficient conservative route clearance: {min_clearance}"
    );
    assert_eq!(peak, 25);
    println!(
        "peak_chunks={peak},minimum_actual_clearance={min_clearance:.3},zero_cells=true,zero_pressure=true"
    );
}
