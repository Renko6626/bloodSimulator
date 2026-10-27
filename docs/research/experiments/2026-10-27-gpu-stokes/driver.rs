//! Captures real sequential RHSs; capture I/O is not a performance measurement.
use simulation::{Config, MoveInput, Simulation};

fn main() {
    let frames: usize = std::env::args().nth(1).unwrap().parse().unwrap();
    let mut sim = Simulation::new_local_world(Config::default());
    let targets = [
        [-13.0, 0.0],
        [-10.0, -4.0],
        [-6.0, -8.0],
        [-10.0, -4.0],
        [-16.0, 0.0],
    ];
    let mut target = 0;
    for _ in 0..frames {
        let state = sim.snapshot();
        let mut delta = [
            targets[target][0] - state.robot[0],
            targets[target][1] - state.robot[1],
        ];
        if delta[0].hypot(delta[1]) < 0.6 {
            target = (target + 1) % targets.len();
            delta = [
                targets[target][0] - state.robot[0],
                targets[target][1] - state.robot[1],
            ];
        }
        let length = delta[0].hypot(delta[1]).max(1.0);
        sim.tick(MoveInput {
            x: delta[0] / length,
            y: delta[1] / length,
        });
        let state = sim.snapshot();
        assert!(
            state
                .cells
                .iter()
                .flatten()
                .flatten()
                .all(|v| v.is_finite())
        );
        let stats = state.fluid.unwrap();
        assert!(stats.divergence_rms < 1e-8 && stats.momentum_rms < 1e-8);
    }
    println!("captured {frames} frames; known wide-domain internal pump omission unchanged");
}
