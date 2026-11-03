//! Four explicitly local initializations; this is not a full-map spawn traversal.
use rapier2d::prelude::Vector;
use simulation::{Config, MoveInput, Simulation, map::fluid_policy::BackgroundSource};

fn main() -> Result<(), String> {
    let template = Simulation::try_new_organ_world(Config::default())?;
    for (name, x) in [
        ("systemic_in", 488.),
        ("pulmonary_out", 528.),
        ("pulmonary_in", 562.),
        ("systemic_out", 600.),
    ] {
        let mut map = template.world_map().unwrap().clone();
        map.spawn = [x, 660.];
        let mut sim = Simulation::try_new_policy_world(template.config.clone(), map)?;
        let mut last = sim.policy_fluid_status().unwrap().background;
        let mut switches = 0;
        let mut steps = 0;
        let mut clearance = f32::INFINITY;
        for target in [652., 660.] {
            let mut reached = false;
            for _ in 0..300 {
                let state = sim.snapshot();
                if (state.robot[1] - target).abs() <= 0.08 {
                    reached = true;
                    break;
                }
                sim.try_tick(MoveInput {
                    x: 0.,
                    y: (target - state.robot[1]).signum(),
                })?;
                let state = sim.snapshot();
                assert!(state.robot.iter().all(|p| p.is_finite()));
                assert!(state.cells.is_empty());
                assert_eq!(state.immunity.white_bodies, 0);
                assert!(!sim.policy_fluid_status().unwrap().window_active);
                let c = -sim
                    .world_map()
                    .unwrap()
                    .signed_distance(Vector::from_array(state.robot));
                clearance = clearance.min(c);
                assert!(
                    c >= 1.,
                    "{name}: insufficient clearance at {:?}",
                    state.robot
                );
                let source = sim.policy_fluid_status().unwrap().background;
                if source != last {
                    switches += 1;
                    last = source;
                }
                steps += 1;
            }
            assert!(reached, "{name}: did not reach local target {target}");
            assert_eq!(
                last,
                if target < 656. {
                    BackgroundSource::Analytic
                } else {
                    BackgroundSource::Precomputed(simulation::map::fluid_policy::FieldId(0))
                }
            );
        }
        assert_eq!(switches, 2);
        println!(
            "case={name} local_start=[{x},660] targets_y=[652,660] steps={steps} source_switches={switches} min_center_clearance={clearance:.6} zero_cells=true lbm=false"
        );
    }
    Ok(())
}
