use super::*;
use crate::white_cell::{ARRIVAL_TIME, Arrival, Goal, WANTED_THRESHOLD, WhiteCell};

impl Simulation {
    pub(super) fn update_white_intents(&mut self) {
        self.immunity.advance(self.config.dt);
        let robot = self.physics.bodies[self.robot].translation();
        let radius = self.immunity.sense_radius();
        for cell in &mut self.cells {
            let Some(white) = &mut cell.white else {
                continue;
            };
            if cell.dead_age.is_some() {
                white.movement = Vector::ZERO;
                white.contact_time = 0.0;
                white.call_time = 0.0;
                continue;
            }
            let center = fluid::center(&self.physics.soft_bodies[cell.body]);
            let visible = center.distance(robot) <= radius
                && crate::combat::clear_path(&self.physics, center, robot);
            white.think(
                center,
                visible.then_some(robot),
                self.immunity.alertness,
                self.config.dt,
                &self.config.white_cells,
            );
        }
    }

    pub(super) fn react_to_hits(&mut self, hits: &[crate::combat::Hit], origin: Vector) {
        if hits.is_empty() {
            return;
        }
        let radius = self.immunity.sense_radius();
        let alertness = self.immunity.alertness;
        for cell in &mut self.cells {
            let Some(white) = &mut cell.white else {
                continue;
            };
            if cell.dead_age.is_some() {
                continue;
            }
            let center = fluid::center(&self.physics.soft_bodies[cell.body]);
            let sees_robot = center.distance(origin) <= radius
                && crate::combat::clear_path(&self.physics, center, origin);
            for hit in hits {
                let direct = hit.id == cell.id;
                let sees_hit = center.distance(hit.position) <= radius
                    && crate::combat::clear_path(&self.physics, center, hit.position);
                if direct || (sees_hit && sees_robot) {
                    white.pursue(origin, alertness);
                } else if sees_hit && !white.is_pursuing() {
                    white.goal = Some(Goal::Investigate(hit.position));
                    white.memory = 3.0;
                }
            }
        }
        let increase: f32 = hits
            .iter()
            .map(|hit| {
                hit.damage * if hit.white { 0.12 } else { 0.04 }
                    + if hit.killed {
                        if hit.white { 10.0 } else { 4.0 }
                    } else {
                        0.0
                    }
            })
            .sum();
        self.immunity.alertness = (self.immunity.alertness + increase).min(100.0);
        self.immunity.quiet_time = 0.0;
    }

    /// Uniform, finite velocity response preserves the soft body's deformation modes.
    pub(super) fn drive_white_cells(&mut self, dt: f32) {
        for index in 0..self.cells.len() {
            let cell = &self.cells[index];
            let Some(white) = &cell.white else { continue };
            if cell.dead_age.is_some() {
                continue;
            }
            let movement = white.movement;
            let handle = cell.body;
            let body = &self.physics.soft_bodies[handle];
            let center = fluid::center(body);
            let target = self.flow_velocity_at(center, fluid::body_radius(body)) + movement;
            let body = &mut self.physics.soft_bodies[handle];
            let mean = body.particle_velocities().sum::<Vector>() / body.num_particles() as f32;
            let delta = (target - mean) * (1.0 - (-30.0 * dt).exp());
            for node in 0..body.num_particles() {
                body.set_particle_velocity(node, body.particle_velocity(node) + delta);
            }
        }
    }

    pub(super) fn finish_white_actions(&mut self) {
        let robot = self.physics.bodies[self.robot].translation();
        let radius = self.immunity.sense_radius();
        let options = &self.config.white_cells;
        let dt = self.config.dt;
        let mut damage = 0.0;
        let mut call = None;
        for cell in &mut self.cells {
            let Some(white) = &mut cell.white else {
                continue;
            };
            if cell.dead_age.is_some() {
                continue;
            }
            let body = &self.physics.soft_bodies[cell.body];
            let center = fluid::center(body);
            let visible = center.distance(robot) <= radius
                && crate::combat::clear_path(&self.physics, center, robot);
            let engaged = white.is_pursuing() && visible;
            // Robot radius 0.85 plus soft contact thickness and a small contact margin.
            let contact = engaged && crate::combat::contact_point(body, robot, 1.05).is_some();
            if contact {
                white.contact_time += dt;
                if white.contact_time + 1.0e-6 >= options.contact_warmup
                    && white.attack_cooldown <= 1.0e-6
                {
                    damage += options.damage;
                    white.attack_cooldown = options.attack_interval;
                }
            } else {
                white.contact_time = 0.0;
            }
            if engaged
                && self.immunity.alertness >= WANTED_THRESHOLD
                && self.immunity.call_cooldown <= 1.0e-6
            {
                white.call_time += dt;
                if white.call_time + 1.0e-6 >= options.call_time && call.is_none() {
                    call = Some((center, robot));
                }
            } else {
                white.call_time = 0.0;
            }
        }
        self.robot_health = (self.robot_health - damage).max(0.0);
        if self.robot_health == 0.0 {
            self.paused = true;
            return;
        }
        if let Some((caller, last_seen)) = call {
            self.immunity.call_cooldown = self.config.white_cells.call_interval;
            for cell in &mut self.cells {
                if let Some(white) = &mut cell.white {
                    white.call_time = 0.0;
                }
            }
            if self.cells.iter().filter(|c| c.white.is_some()).count()
                >= self.config.white_cells.max_bodies
            {
                return;
            }
            if let Some((position, wall)) = self.reinforcement_position(caller) {
                let index = self.insert_white_cell(position);
                let white = self.cells[index].white.as_mut().unwrap();
                white.goal = Some(Goal::Investigate(last_seen));
                white.memory = 3.0;
                white.arrival_time = ARRIVAL_TIME;
                self.immunity.arrival = Some(Arrival {
                    wall,
                    position,
                    remaining: ARRIVAL_TIME,
                });
            }
        }
    }

    pub(super) fn insert_white_cell(&mut self, position: Vector) -> usize {
        self.white_cells_enabled = true;
        let body = cell::insert_with_radius(
            &mut self.physics,
            position,
            self.config.white_cells.radius,
            &self.config,
        );
        let id = self.next_cell_id;
        self.next_cell_id += 1;
        let mut cell = cell::Cell::new(id, body, self.config.white_cells.health);
        cell.white = Some(WhiteCell::new(id));
        self.cells.push(cell);
        self.reset_local_cell_history();
        self.cells.len() - 1
    }

    pub(super) fn seed_white_cells(&mut self) {
        let Some(map) = self.world_map() else { return };
        let edges = map.edges.len();
        let start = map.nearest_edge(self.physics.bodies[self.robot].translation());
        let count = self
            .config
            .white_cells
            .count
            .min(self.config.white_cells.max_bodies);
        for slot in 0..count {
            let mut found = None;
            for offset in 0..edges {
                let edge = (start + slot * edges / count + offset) % edges;
                let length = self.world_map().unwrap().edge_length(edge);
                for step in 0..length.ceil() as usize {
                    let along = 2.0 + step as f32 * 1.3;
                    if along > length - 2.0 {
                        break;
                    }
                    for side in [-1.0, 1.0] {
                        if let Some((position, _)) = self.white_spawn_candidate(edge, along, side) {
                            found = Some(position);
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            if let Some(position) = found {
                self.insert_white_cell(position);
            }
        }
    }

    fn reinforcement_position(&self, caller: Vector) -> Option<(Vector, Vector)> {
        let map = self.world_map()?;
        let edge = map.nearest_edge(caller);
        let along = map.project_edge(edge, caller).along;
        for offset in [3.0, -3.0, 5.0, -5.0, 7.0, -7.0] {
            let candidate = along + offset;
            if candidate < 1.5 || candidate > map.edge_length(edge) - 1.5 {
                continue;
            }
            for side in [-1.0, 1.0] {
                if let Some(point) = self.white_spawn_candidate(edge, candidate, side) {
                    return Some(point);
                }
            }
        }
        None
    }

    fn white_spawn_candidate(
        &self,
        edge: usize,
        along: f32,
        side: f32,
    ) -> Option<(Vector, Vector)> {
        let map = self.world_map()?;
        let radius = self.config.white_cells.radius;
        let half_width = map.edges[edge].half_width;
        if half_width <= radius + 0.25 {
            return None;
        }
        let position = map.edge_point(edge, along, side * (half_width - radius - 0.3));
        if map.signed_distance(position) > -radius - 0.25
            || map.nearest_edge(position) != edge
            || position.distance(self.physics.bodies[self.robot].translation()) < radius + 1.35
            || self.cells.iter().any(|cell| {
                let (min, max) = cell_bounds(&self.physics.soft_bodies[cell.body]);
                let separation = (min - position).max(position - max).max(Vector::ZERO);
                separation.length() < radius + 0.15
            })
        {
            return None;
        }
        Some((position, map.edge_point(edge, along, side * half_width)))
    }

    pub(super) fn reset_local_cell_history(&mut self) {
        if let Some(local) = &mut self.local_fluid {
            local.reset_history(self.cells.len());
        }
    }

    pub(super) fn flow_velocity_at(&self, position: Vector, radius: f32) -> Vector {
        let mut velocity = self.environment.velocity_at(position);
        if let Some(fluid) = &self.local_fluid {
            let weight = fluid.weight(position, radius);
            velocity = velocity * (1.0 - weight) + fluid.sample(position) * weight;
        }
        #[cfg(feature = "lbm-gpu")]
        if let (Some(lbm), Environment::World { map, .. }) = (&self.lbm, &self.environment) {
            velocity = composed_velocity(lbm.sample_world(map, position, velocity));
        }
        velocity
    }
}
