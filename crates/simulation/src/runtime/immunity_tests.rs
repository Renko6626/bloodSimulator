use super::*;
use crate::white_cell::Goal;

fn scene() -> Simulation {
    let mut sim = population_tests::seeded();
    sim.config.white_cells.patrol_speed = 0.0;
    sim.config.white_cells.investigate_speed = 0.0;
    sim.config.white_cells.chase_speed = 0.0;
    sim
}

fn frames(sim: &mut Simulation, count: usize) {
    for _ in 0..count {
        sim.tick(MoveInput::default());
    }
}

#[test]
fn actual_player_damage_alerts_witnesses_and_quiet_time_reduces_alertness() {
    let mut sim = scene();
    let white = sim.insert_white_cell(Vector::new(8.0, 0.0));
    sim.physics.bodies[sim.robot].set_translation(Vector::new(3.0, 0.0), true);
    sim.set_attacking(true);
    frames(&mut sim, 1);
    assert_eq!(sim.immunity.alertness, 2.0);
    assert!(sim.cells[white].white.as_ref().unwrap().is_pursuing());
    frames(&mut sim, 23);
    assert_eq!(sim.immunity.alertness, 8.0);
    sim.set_attacking(false);
    frames(&mut sim, 420);
    assert!((sim.immunity.alertness - 6.0).abs() < 0.15);
}

#[test]
fn walls_hide_the_robot_and_pursuit_uses_only_the_last_known_position() {
    let mut sim = scene();
    let white = sim.insert_white_cell(Vector::new(9.0, 0.0));
    sim.physics.bodies[sim.robot].set_translation(Vector::new(2.0, 0.0), true);
    sim.immunity.alertness = 100.0;
    frames(&mut sim, 1);
    let known = sim.physics.bodies[sim.robot].translation();
    sim.physics.insert_collider(
        ColliderBuilder::cuboid(0.05, 2.5).translation(Vector::new(6.0, 0.0)),
        None,
    );
    sim.physics.step();
    sim.physics.bodies[sim.robot].set_translation(Vector::new(1.0, 0.0), true);
    frames(&mut sim, 1);
    assert_eq!(
        sim.cells[white].white.as_ref().unwrap().goal,
        Some(Goal::Pursue(known))
    );
    frames(&mut sim, 305);
    assert_eq!(sim.cells[white].white.as_ref().unwrap().goal, None);
}

#[test]
fn contact_requires_preparation_and_lethal_player_attack_cancels_the_return_hit() {
    let mut sim = scene();
    let white = sim.insert_white_cell(Vector::new(-5.0, 0.0));
    sim.physics.bodies[sim.robot].set_translation(Vector::new(-3.0, 0.0), true);
    sim.set_attacking(true);
    frames(&mut sim, 1);
    sim.set_attacking(false);
    frames(&mut sim, 10);
    assert_eq!(sim.robot_health, 100.0);
    frames(&mut sim, 9);
    assert_eq!(sim.robot_health, 92.0);
    sim.physics.bodies[sim.robot].set_translation(Vector::ZERO, true);
    frames(&mut sim, 1);
    assert_eq!(sim.cells[white].white.as_ref().unwrap().contact_time, 0.0);
    sim.physics.bodies[sim.robot].set_translation(Vector::new(-3.0, 0.0), true);
    sim.cells[white].health = 50.0;
    let brain = sim.cells[white].white.as_mut().unwrap();
    brain.contact_time = 0.3;
    brain.attack_cooldown = 0.0;
    sim.attack.cooldown = 0.0;
    sim.set_attacking(true);
    frames(&mut sim, 1);
    assert!(sim.cells[white].dead_age.is_some());
    assert_eq!(sim.robot_health, 92.0);

    // A destroyed robot cannot be resumed or single-stepped back into play.
    sim.set_attacking(false);
    let other = sim.insert_white_cell(Vector::new(-1.0, 0.0));
    sim.cells[other]
        .white
        .as_mut()
        .unwrap()
        .pursue(Vector::new(-3.0, 0.0), 0.0);
    sim.config.white_cells.damage = 100.0;
    frames(&mut sim, 25);
    assert_eq!(sim.robot_health, 0.0);
    let steps = sim.steps;
    sim.set_paused(false);
    sim.single_step(MoveInput::default());
    frames(&mut sim, 1);
    assert_eq!(sim.steps, steps);
    sim.reset();
    assert_eq!(sim.robot_health, 100.0);
    assert_eq!(sim.immunity.alertness, 0.0);
}

#[test]
fn reinforcements_are_bounded_and_white_bodies_stay_out_of_red_stock() {
    let mut sim = scene();
    let first = sim.insert_white_cell(Vector::new(9.0, 0.0));
    sim.immunity.alertness = 100.0;
    frames(&mut sim, 62);
    assert_eq!(sim.cells.iter().filter(|c| c.white.is_some()).count(), 2);
    assert!(sim.immunity.arrival.is_some());
    let white_handles: Vec<_> = sim
        .cells
        .iter()
        .filter(|c| c.white.is_some())
        .map(|c| c.body)
        .collect();
    for handle in white_handles {
        assert!(
            sim.physics.soft_bodies[handle]
                .particle_positions()
                .all(|p| sim.world_map().unwrap().signed_distance(p) < 0.0)
        );
    }
    frames(&mut sim, 60);
    assert_eq!(sim.cells.iter().filter(|c| c.white.is_some()).count(), 2);
    for x in [25.0, 30.0, 35.0, 40.0, 45.0, 50.0] {
        sim.insert_white_cell(Vector::new(x, 0.0));
    }
    sim.immunity.call_cooldown = 0.0;
    sim.cells[first].white.as_mut().unwrap().call_time = 1.0;
    frames(&mut sim, 1);
    assert_eq!(sim.cells.iter().filter(|c| c.white.is_some()).count(), 8);
    sim.physics.bodies[sim.robot].set_translation(Vector::new(-70.0, 0.0), true);
    sim.update_cell_population();
    let state = sim.snapshot();
    assert_eq!(state.immunity.white_bodies, 8);
    let population = state.population.unwrap();
    assert_eq!(population.detailed_count, 0);
    assert_eq!(population.total_count, 11);
    assert!((population.abstract_count - 11.0).abs() < 1.0e-10);
}

#[test]
fn a_pursuing_white_cell_really_moves_with_zero_background_flow() {
    let mut sim = scene();
    sim.config.white_cells.chase_speed = 3.0;
    sim.config.white_cells.call_time = 100.0;
    let white = sim.insert_white_cell(Vector::new(-8.0, 0.0));
    let body = sim.cells[white].body;
    sim.physics.bodies[sim.robot].set_translation(Vector::new(-3.0, 0.0), true);
    sim.immunity.alertness = 100.0;
    frames(&mut sim, 90);
    assert!(fluid::center(&sim.physics.soft_bodies[body]).x > -7.0);
    assert!(
        sim.physics.soft_bodies[body]
            .particle_positions()
            .all(|p| p.is_finite())
    );
}
