//! Bounded map-resource replay. No red/white population; not an FPS benchmark.
use simulation::{
    Config, MoveInput, Simulation, WhiteCellOptions,
    map::{MapDefinition, VesselEdge},
};
use std::time::Instant;

fn main() {
    println!("span,startup_ms,initial_walls,peak_chunks,final_walls,final_x,final_y");
    for size in [300., 1000., 3000.] {
        let nodes = vec![[0., 24.], [size, 24.], [size, size], [0., size]];
        let edges = (0..4)
            .map(|from| {
                let to = (from + 1) % 4;
                VesselEdge {
                    from,
                    to,
                    centerline: vec![nodes[from], nodes[to]],
                    half_width: 3.,
                }
            })
            .collect();
        let map = MapDefinition {
            fluid: None,
            nodes,
            edges,
            pump_edge: 3,
            spawn: [24., 24.],
            landmarks: vec![],
        };
        let config = Config {
            cell_count: 0,
            white_cells: WhiteCellOptions {
                count: 0,
                ..Default::default()
            },
            pressure: 0.,
            robot_flow: 0.,
            robot_speed: 8.,
            ..Default::default()
        };
        let start = Instant::now();
        let mut sim = Simulation::new_streamed_world(config, map);
        let startup_ms = start.elapsed().as_secs_f64() * 1000.;
        let initial = sim.snapshot().streaming.unwrap();
        let mut peak = initial.loaded_chunks;
        for frame in 0..2400 {
            let state = sim.snapshot();
            let target = if frame < 1200 { 144. } else { 24. };
            sim.sync_world_view(
                [
                    [state.robot[0] - 14., state.robot[1] - 9.],
                    [state.robot[0] + 14., state.robot[1] + 9.],
                ],
                false,
            )
            .unwrap();
            sim.tick(MoveInput {
                x: ((target - state.robot[0]) * 2.).clamp(-1., 1.),
                y: 0.,
            });
            peak = peak.max(sim.snapshot().streaming.unwrap().loaded_chunks);
        }
        let state = sim.snapshot();
        let stats = state.streaming.unwrap();
        assert_eq!(peak, 25);
        assert_eq!(stats.wall_colliders, initial.wall_colliders);
        assert!((state.robot[0] - 24.).abs() < 0.05);
        assert!(sim.world_geometry().is_none());
        println!(
            "{size},{startup_ms:.3},{},{peak},{},{:.4},{:.4}",
            initial.wall_colliders, stats.wall_colliders, state.robot[0], state.robot[1]
        );
    }
}
