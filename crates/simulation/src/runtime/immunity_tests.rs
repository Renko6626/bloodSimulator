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
    assert!(sim.cells[white].behavior.as_ref().unwrap().is_pursuing());
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
        sim.cells[white].behavior.as_ref().unwrap().goal,
        Some(Goal::Pursue(known))
    );
    frames(&mut sim, 305);
    assert_eq!(sim.cells[white].behavior.as_ref().unwrap().goal, None);
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
    assert_eq!(sim.robot_core.health, 100.0);
    frames(&mut sim, 9);
    assert_eq!(sim.robot_core.health, 92.0);
    sim.physics.bodies[sim.robot].set_translation(Vector::ZERO, true);
    frames(&mut sim, 1);
    assert_eq!(
        sim.cells[white].behavior.as_ref().unwrap().contact_time,
        0.0
    );
    sim.physics.bodies[sim.robot].set_translation(Vector::new(-3.0, 0.0), true);
    sim.cells[white].health = 50.0;
    let brain = sim.cells[white].behavior.as_mut().unwrap();
    brain.contact_time = 0.3;
    brain.attack_cooldown = 0.0;
    sim.attack.cooldown = 0.0;
    sim.set_attacking(true);
    frames(&mut sim, 1);
    assert!(sim.cells[white].dead_age.is_some());
    assert_eq!(sim.robot_core.health, 92.0);

    // A destroyed robot cannot be resumed or single-stepped back into play.
    sim.set_attacking(false);
    let other = sim.insert_white_cell(Vector::new(-1.0, 0.0));
    sim.cells[other]
        .behavior
        .as_mut()
        .unwrap()
        .pursue(Vector::new(-3.0, 0.0), 0.0);
    sim.config.white_cells.damage = 100.0;
    frames(&mut sim, 25);
    assert_eq!(sim.robot_core.health, 0.0);
    let steps = sim.steps;
    sim.set_paused(false);
    sim.single_step(MoveInput::default());
    frames(&mut sim, 1);
    assert_eq!(sim.steps, steps);
    sim.reset();
    assert_eq!(sim.robot_core.health, 100.0);
    assert_eq!(sim.immunity.alertness, 0.0);
}

#[test]
fn reinforcements_are_bounded_and_white_bodies_stay_out_of_red_stock() {
    let mut sim = scene();
    let first = sim.insert_white_cell(Vector::new(9.0, 0.0));
    sim.immunity.alertness = 100.0;
    frames(&mut sim, 62);
    assert_eq!(
        sim.cells.iter().filter(|c| c.species.is_immune()).count(),
        2
    );
    assert!(sim.immunity.arrival.is_some());
    let white_handles: Vec<_> = sim
        .cells
        .iter()
        .filter(|c| c.species.is_immune())
        .filter_map(|c| c.active_body())
        .collect();
    for handle in white_handles {
        assert!(
            sim.physics.soft_bodies[handle]
                .particle_positions()
                .all(|p| sim.world_map().unwrap().signed_distance(p) < 0.0)
        );
    }
    frames(&mut sim, 60);
    assert_eq!(
        sim.cells.iter().filter(|c| c.species.is_immune()).count(),
        2
    );
    for x in [25.0, 30.0, 35.0, 40.0, 45.0, 50.0] {
        sim.insert_white_cell(Vector::new(x, 0.0));
    }
    sim.immunity.call_cooldown = 0.0;
    sim.cells[first].behavior.as_mut().unwrap().call_time = 1.0;
    frames(&mut sim, 1);
    assert_eq!(
        sim.cells.iter().filter(|c| c.species.is_immune()).count(),
        8
    );
    sim.physics.bodies[sim.robot].set_translation(Vector::new(-70.0, 0.0), true);
    sim.update_cell_population();
    let state = sim.snapshot();
    assert_eq!(state.immunity.immune_bodies, 8);
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
    let body = sim.cells[white].active_body().unwrap();
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

// New species use these same real bodies, flow, collision and combat coordinator.
fn combat_pair(species: crate::Species) -> (Simulation, usize, usize) {
    let mut sim = scene();
    let immune = sim.insert_white_cell(Vector::new(-7.0, 0.0));
    let pathogen = sim.insert_species(Vector::new(-5.5, 0.0), species);
    (sim, immune, pathogen)
}

#[test]
fn neutral_robot_can_watch_real_immune_bacteria_combat_without_becoming_wanted() {
    let (mut sim, immune, pathogen) = combat_pair(crate::Species::MotileBacterium);
    sim.config.white_cells.chase_speed = 3.0;
    sim.cells[pathogen].behavior.as_mut().unwrap().heading = -Vector::X;
    frames(&mut sim, 150);
    assert!(sim.cells[pathogen].health < 50.0);
    assert!(sim.cells[immune].health < 150.0);
    assert_eq!(sim.immunity.alertness, 0.0);
    assert_eq!(sim.robot_core.health, 100.0);
    assert!(
        !sim.cells[immune]
            .behavior
            .as_ref()
            .unwrap()
            .target
            .is_some_and(|t| t == crate::entity::TargetRef::Robot)
    );
    for status in sim.snapshot().cells {
        assert!(status.into_iter().flatten().all(f32::is_finite));
    }
}

#[test]
fn capsule_halves_immune_contact_damage_but_player_damage_has_no_wanted_score() {
    let (mut sim, immune, pathogen) = combat_pair(crate::Species::EncapsulatedBacterium);
    // Bodies stay in their actual initial contact while testing the action stage.
    let target = crate::entity::TargetRef::Cell(sim.cells[pathogen].id);
    sim.cells[immune]
        .behavior
        .as_mut()
        .unwrap()
        .set_target(Some(target));
    for _ in 0..18 {
        sim.finish_white_actions();
    }
    assert_eq!(sim.cells[pathogen].health, 96.0);
    sim.physics.bodies[sim.robot].set_translation(Vector::new(-4.0, 0.0), true);
    sim.set_attacking(true);
    sim.advance_attack();
    assert_eq!(sim.cells[pathogen].health, 46.0);
    assert_eq!(sim.immunity.alertness, 0.0);
    // The same actual player pulse still scores damage against normal cells.
    sim.physics.bodies[sim.robot].set_translation(Vector::new(3.0, 0.0), true);
    sim.attack.cooldown = 0.0;
    sim.advance_attack();
    assert_eq!(sim.immunity.alertness, 2.0);
}

#[test]
fn lost_target_and_switching_contact_require_fresh_preparation() {
    let (mut sim, immune, pathogen) = combat_pair(crate::Species::MotileBacterium);
    sim.update_white_intents();
    for _ in 0..12 {
        sim.finish_white_actions();
    }
    assert!(sim.cells[immune].behavior.as_ref().unwrap().contact_time > 0.19);
    let old_position =
        fluid::center(&sim.physics.soft_bodies[sim.cells[pathogen].active_body().unwrap()]);
    sim.cells[pathogen].hibernate(&mut sim.physics);
    sim.update_white_intents();
    sim.finish_white_actions();
    let brain = sim.cells[immune].behavior.as_ref().unwrap();
    assert_eq!(brain.contact_time, 0.0);
    assert_eq!(brain.goal, Some(Goal::Pursue(old_position)));
    let replacement = sim.insert_species(
        Vector::new(-5.5, 0.0),
        crate::Species::EncapsulatedBacterium,
    );
    sim.update_white_intents();
    for _ in 0..12 {
        sim.finish_white_actions();
    }
    assert_eq!(sim.cells[replacement].health, 100.0);
    assert_eq!(
        sim.cells[immune].behavior.as_ref().unwrap().target,
        Some(crate::entity::TargetRef::Cell(sim.cells[replacement].id))
    );
    for _ in 0..6 {
        sim.finish_white_actions();
    }
    assert_eq!(sim.cells[replacement].health, 96.0);
}

#[test]
fn simultaneous_biological_kills_and_player_first_cancellation_use_real_contacts() {
    let (mut sim, immune, pathogen) = combat_pair(crate::Species::MotileBacterium);
    sim.cells[immune].health = 5.0;
    sim.cells[pathogen].health = 8.0;
    let immune_id = sim.cells[immune].id;
    let pathogen_id = sim.cells[pathogen].id;
    for (index, target) in [(immune, pathogen_id), (pathogen, immune_id)] {
        let brain = sim.cells[index].behavior.as_mut().unwrap();
        brain.set_target(Some(crate::entity::TargetRef::Cell(target)));
        brain.contact_time = 1.0;
    }
    sim.finish_white_actions();
    assert_eq!(sim.cells[immune].health, 0.0);
    assert_eq!(sim.cells[pathogen].health, 0.0);
    assert_eq!(sim.immunity.alertness, 0.0);

    let (mut sim, immune, pathogen) = combat_pair(crate::Species::MotileBacterium);
    let target = crate::entity::TargetRef::Cell(sim.cells[immune].id);
    let brain = sim.cells[pathogen].behavior.as_mut().unwrap();
    brain.set_target(Some(target));
    brain.contact_time = 1.0;
    sim.physics.bodies[sim.robot].set_translation(Vector::new(-4.0, 0.0), true);
    sim.set_attacking(true);
    sim.advance_attack();
    sim.finish_white_actions();
    assert_eq!(sim.cells[pathogen].health, 0.0);
    assert_eq!(sim.cells[immune].health, 150.0);
}

#[test]
fn new_species_hibernate_with_identity_and_never_enter_red_inventory() {
    let mut sim = Simulation::new_streamed_world(
        Config {
            cell_count: 0,
            cell_mode: CellSimulationMode::Local(Default::default()),
            pressure: 0.0,
            white_cells: crate::WhiteCellOptions {
                count: 0,
                ..Default::default()
            },
            ..Default::default()
        },
        crate::map::MapDefinition::small_world(),
    );
    let species = [
        crate::Species::Macrophage,
        crate::Species::MotileBacterium,
        crate::Species::EncapsulatedBacterium,
    ];
    for (i, species) in species.into_iter().enumerate() {
        let index = sim.insert_species(Vector::new(i as f32 * 4.0, -12.0), species);
        sim.cells[index].damage(10.0);
        sim.cells[index].behavior.as_mut().unwrap().contact_time = 0.2;
    }
    let before: Vec<_> = sim
        .cells
        .iter()
        .map(|c| (c.id, c.species, c.health))
        .collect();
    sim.physics.bodies[sim.robot].set_translation(Vector::new(80., 0.), true);
    sim.update_cell_population();
    sim.update_streaming();
    assert!(sim.snapshot().cells.is_empty());
    assert_eq!(sim.snapshot().immunity.pathogen_bodies, 0);
    sim.physics.bodies[sim.robot].set_translation(Vector::new(-20., 0.), true);
    sim.update_streaming();
    assert_eq!(
        sim.cells
            .iter()
            .map(|c| (c.id, c.species, c.health))
            .collect::<Vec<_>>(),
        before
    );
    for cell in &sim.cells {
        assert_eq!(cell.behavior.as_ref().unwrap().contact_time, 0.0);
        let body = &sim.physics.soft_bodies[cell.active_body().unwrap()];
        let area = body.volume();
        let radius = cell.species.radius(&sim.config);
        assert!((area - 3.0 * radius * radius).abs() < 1e-3);
    }
    sim.cells[1].damage(1000.0);
    frames(&mut sim, 365);
    assert_eq!(
        sim.cells
            .iter()
            .filter(|c| c.species == crate::Species::RedCell)
            .count(),
        0
    );
    let population = sim.population.as_ref().unwrap();
    assert_eq!(population.total_count, 0);
    assert_eq!(population.abstract_count(), 0.0);
}

#[test]
fn preview_reset_and_heavy_reinforcements_keep_scene_and_body_limits() {
    let mut sim = Simulation::new_combat_preview(Config::default());
    let initial = sim.snapshot();
    assert_eq!(initial.immunity.immune_bodies, 4);
    assert_eq!(initial.immunity.pathogen_bodies, 6);
    assert_eq!(initial.cells.len(), 82);
    assert!(matches!(sim.config.cell_mode, CellSimulationMode::Full));
    for cell in &sim.cells {
        let body = &sim.physics.soft_bodies[cell.active_body().unwrap()];
        assert!(
            body.particle_positions()
                .all(|p| p.is_finite() && sim.world_map().unwrap().signed_distance(p) < 0.0)
        );
    }
    frames(&mut sim, 120);
    sim.reset();
    assert_eq!(sim.snapshot().cells, initial.cells);
    assert_eq!(sim.snapshot().cell_status, initial.cell_status);
    sim.immunity.alertness = 100.0;
    let heavy = sim
        .cells
        .iter()
        .position(|c| c.species == crate::Species::Macrophage)
        .unwrap();
    sim.cells[heavy].damage(1000.0);
    sim.cells[heavy].hibernate(&mut sim.physics);
    sim.try_reinforcement(Vector::new(-14., 0.), Vector::new(-20., 0.));
    assert_eq!(
        sim.cells
            .iter()
            .filter(|c| c.species == crate::Species::Macrophage)
            .count(),
        1
    );
    // Even a dormant corpse occupies the heavy slot; remove it to test a new call.
    let index = sim
        .cells
        .iter()
        .position(|c| c.species == crate::Species::Macrophage)
        .unwrap();
    let corpse = sim.cells.remove(index);
    if let Some(handle) = corpse.active_body() {
        sim.physics.remove_soft_body(handle);
    }
    // A clear upper branch has sufficient wall clearance for the heavy unit.
    sim.try_reinforcement(Vector::new(6., -12.), Vector::new(9., -12.));
    assert_eq!(
        sim.cells
            .iter()
            .filter(|c| c.species == crate::Species::Macrophage)
            .count(),
        1
    );
    assert!(sim.immunity.call_cooldown > 0.0);
    let mut ordinary = scene();
    ordinary.immunity.alertness = 100.0;
    ordinary.try_reinforcement(Vector::new(8., 0.), Vector::new(0., 0.));
    assert!(
        ordinary
            .cells
            .iter()
            .all(|c| c.species != crate::Species::Macrophage && !c.species.is_pathogen())
    );
}

#[test]
fn abandoned_attackers_do_not_override_current_or_neutral_targets() {
    let mut sim = scene();
    let immune = sim.insert_white_cell(Vector::new(-5.0, 0.0));
    sim.physics.bodies[sim.robot].set_translation(Vector::new(-3.0, 0.0), true);
    sim.set_attacking(true);
    frames(&mut sim, 1);
    sim.set_attacking(false);
    assert_eq!(sim.cells[immune].health, 100.0);
    assert_eq!(
        sim.cells[immune].behavior.as_ref().unwrap().attacker,
        Some(crate::entity::TargetRef::Robot)
    );

    let pathogen = sim.insert_species(
        Vector::new(-1.5, 0.0),
        crate::Species::EncapsulatedBacterium,
    );
    let target = crate::entity::TargetRef::Cell(sim.cells[pathogen].id);
    sim.physics.bodies[sim.robot].set_translation(Vector::new(30.0, 0.0), true);
    // The immune cell switches to the visible pathogen while the robot's old
    // evidence and wanted score have ample time to expire.
    frames(&mut sim, 720);
    assert_eq!(sim.immunity.alertness, 0.0);
    assert_eq!(
        sim.cells[immune].behavior.as_ref().unwrap().target,
        Some(target)
    );
    assert!(sim.cells[pathogen].health > 0.0);
    sim.physics.bodies[sim.robot].set_translation(Vector::new(-3.0, 0.0), true);
    frames(&mut sim, 1);
    assert_eq!(
        sim.cells[immune].behavior.as_ref().unwrap().target,
        Some(target)
    );
    assert_eq!(sim.cells[immune].behavior.as_ref().unwrap().attacker, None);

    // A bacterium also forgets an immune attacker once no legal target remains.
    let (mut sim, immune, pathogen) = combat_pair(crate::Species::EncapsulatedBacterium);
    sim.update_white_intents();
    for _ in 0..18 {
        sim.finish_white_actions();
    }
    assert_eq!(sim.cells[pathogen].health, 96.0);
    assert_eq!(
        sim.cells[pathogen].behavior.as_ref().unwrap().attacker,
        Some(crate::entity::TargetRef::Cell(sim.cells[immune].id))
    );
    sim.cells[immune].hibernate(&mut sim.physics);
    sim.update_white_intents();
    assert_eq!(sim.cells[pathogen].behavior.as_ref().unwrap().target, None);
    assert_eq!(
        sim.cells[pathogen].behavior.as_ref().unwrap().attacker,
        None
    );
    sim.cells[immune].restore(&mut sim.physics, &sim.config);
    sim.update_white_intents();
    assert_eq!(sim.cells[pathogen].behavior.as_ref().unwrap().target, None);
}
