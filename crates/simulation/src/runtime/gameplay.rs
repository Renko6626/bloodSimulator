use super::*;

impl Simulation {
    /// Held input; attacks and cooldowns only advance when the simulation steps.
    pub fn set_attacking(&mut self, attacking: bool) {
        self.attack.held = attacking;
    }

    pub(super) fn cell_handles(&self) -> Vec<SoftBodyHandle> {
        self.cells.iter().map(|cell| cell.body).collect()
    }

    pub(super) fn advance_cell_lifetimes(&mut self) {
        let previous_count = self.cells.len();
        self.cells.retain_mut(|cell| {
            if !cell.advance_corpse(&mut self.physics.soft_bodies[cell.body], &self.config) {
                return true;
            }
            self.physics.remove_soft_body(cell.body);
            if let Some(population) = &mut self.population
                && cell.white.is_none()
            {
                population.total_count -= 1;
            }
            false
        });
        if self.cells.len() != previous_count {
            self.reset_local_cell_history();
        }
    }

    pub(super) fn advance_attack(&mut self) {
        let origin = self.physics.bodies[self.robot].translation();
        if !self.attack.advance(origin, &self.config) {
            return;
        }
        let walls = self
            .physics
            .query_pipeline_with_filter(QueryFilter::only_fixed());
        let mut hits = Vec::new();
        for cell in &mut self.cells {
            if cell.dead_age.is_some() {
                continue;
            }
            let body = &self.physics.soft_bodies[cell.body];
            let Some(point) = crate::combat::contact_point(body, origin, self.config.attack_radius)
            else {
                continue;
            };
            let direction = point - origin;
            if direction.length_squared() > 1.0e-8
                && walls
                    .cast_ray(&Ray::new(origin, direction), 1.0, true)
                    .is_some()
            {
                continue;
            }
            let health = cell.health;
            cell.damage(self.config.attack_damage);
            hits.push(crate::combat::Hit {
                id: cell.id,
                position: fluid::center(body),
                white: cell.white.is_some(),
                damage: health - cell.health,
                killed: cell.dead_age.is_some(),
            });
        }
        self.react_to_hits(&hits, origin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near_cell() -> Simulation {
        let mut sim = super::super::population_tests::seeded();
        // Cell center is x=5, outside the 1.9 attack radius; its edge is inside.
        sim.physics.bodies[sim.robot].set_translation(Vector::new(3.0, 0.0), true);
        sim
    }

    #[test]
    fn melee_hits_the_cell_edge_once_per_pulse_and_respects_range() {
        let mut sim = near_cell();
        sim.set_attacking(true);
        sim.tick(MoveInput::default());
        assert_eq!(sim.snapshot().cell_status[0].health, 50.0);
        sim.tick(MoveInput::default());
        assert_eq!(sim.snapshot().cell_status[0].health, 50.0);
        sim.physics.bodies[sim.robot].set_translation(Vector::ZERO, true);
        for _ in 0..24 {
            sim.tick(MoveInput::default());
        }
        assert_eq!(sim.snapshot().cell_status[0].health, 50.0);
        sim.physics.bodies[sim.robot].set_translation(Vector::new(3.0, 0.0), true);
        for _ in 0..24 {
            sim.tick(MoveInput::default());
        }
        assert!(sim.snapshot().cell_status[0].dead);
        assert_eq!(sim.snapshot().cell_status[0].health, 0.0);
    }

    #[test]
    fn fixed_wall_blocks_melee_damage() {
        let mut sim = near_cell();
        sim.physics.insert_collider(
            ColliderBuilder::cuboid(0.05, 2.0).translation(Vector::new(4.0, 0.0)),
            None,
        );
        sim.set_attacking(true);
        sim.tick(MoveInput::default());
        assert_eq!(sim.snapshot().cell_status[0].health, 100.0);
    }

    #[test]
    fn corpse_deflates_as_a_real_soft_body_and_is_eventually_removed() {
        let mut sim = near_cell();
        sim.cells[0].damage(100.0);
        let handle = sim.cells[0].body;
        let original_area = sim.physics.soft_bodies[handle].volume();
        for _ in 0..60 {
            sim.tick(MoveInput::default());
        }
        let body = &sim.physics.soft_bodies[handle];
        let ratio = body.volume() / original_area;
        assert!((0.4..0.8).contains(&ratio), "corpse area ratio {ratio}");
        assert!(body.particle_positions().all(|p| p.is_finite()));
        assert_eq!(sim.physics.soft_bodies.len(), 1);
        assert!(sim.snapshot().cell_status[0].dead);
        for _ in 0..310 {
            sim.tick(MoveInput::default());
        }
        assert!(sim.cells.is_empty());
        assert!(sim.physics.soft_bodies.get(handle).is_none());
        let population = sim.snapshot().population.unwrap();
        assert_eq!(population.total_count, 10);
        assert!((population.abstract_count - 10.0).abs() < 1.0e-10);
    }

    #[test]
    fn a_dead_cell_leaving_the_window_never_returns_as_live_stock() {
        let mut sim = near_cell();
        sim.cells[0].damage(100.0);
        let body = sim.cells[0].body;
        sim.physics.bodies[sim.robot].set_translation(Vector::new(-70.0, 0.0), true);
        sim.update_cell_population();
        assert!(sim.physics.soft_bodies.get(body).is_none());
        assert_eq!(sim.population.as_ref().unwrap().total_count, 10);
        assert!((sim.population.as_ref().unwrap().abstract_count() - 10.0).abs() < 1.0e-10);
    }

    #[test]
    fn pause_single_step_and_reset_control_attacks_and_corpses() {
        let mut sim = near_cell();
        sim.set_paused(true);
        sim.set_attacking(true);
        sim.tick(MoveInput::default());
        assert_eq!(sim.snapshot().cell_status[0].health, 100.0);
        sim.single_step(MoveInput::default());
        assert_eq!(sim.snapshot().cell_status[0].health, 50.0);
        sim.cells[0].damage(100.0);
        let before = sim.snapshot();
        sim.tick(MoveInput::default());
        assert_eq!(before.cell_status, sim.snapshot().cell_status);
        assert_eq!(before.cells, sim.snapshot().cells);
        sim.single_step(MoveInput::default());
        assert!(sim.cells[0].dead_age.unwrap() > 0.0);
        sim.reset();
        let reset = sim.snapshot();
        assert!(reset.paused);
        assert!(reset.attack.is_none());
        assert!(!sim.attack.held);
        assert!(
            reset
                .cell_status
                .iter()
                .all(|c| !c.dead && c.health == 100.0)
        );
    }
}
