use super::*;
use crate::white_cell::{ARRIVAL_TIME, Arrival, Goal, WANTED_THRESHOLD, WhiteCell};
use crate::{Species, entity::TargetRef};

#[derive(Clone, Copy)]
struct Seen {
    target: TargetRef,
    species: Option<Species>,
    position: Vector,
}

impl Simulation {
    fn active_facts(&self) -> Vec<Seen> {
        let mut facts = vec![Seen {
            target: TargetRef::Robot,
            species: None,
            position: self.physics.bodies[self.robot].translation(),
        }];
        facts.extend(
            self.cells
                .iter()
                .filter(|c| c.dead_age.is_none())
                .filter_map(|c| {
                    Some(Seen {
                        target: TargetRef::Cell(c.id),
                        species: Some(c.species),
                        position: fluid::center(&self.physics.soft_bodies[c.active_body()?]),
                    })
                }),
        );
        facts
    }

    /// Fixed encounter data on small_world. The upper and lower branches remain
    /// alternative routes around the macrophage on the upper branch.
    pub fn new_combat_preview(mut config: Config) -> Self {
        config.cell_mode = CellSimulationMode::Full;
        config.cell_count = 72;
        let count = config.white_cells.count;
        config.white_cells.count = 0;
        let mut sim = Self::new_world(config);
        sim.config.white_cells.count = count;
        sim.combat_preview = true;
        let encounters = [
            (Species::Neutrophil, 0, 9.5, 1.),
            (Species::MotileBacterium, 0, 11., 1.),
            (Species::EncapsulatedBacterium, 1, 2., -1.),
            (Species::Neutrophil, 2, 5., 1.),
            (Species::MotileBacterium, 2, 7., 1.),
            (Species::Macrophage, 2, 12., -1.),
            (Species::EncapsulatedBacterium, 2, 14., -1.),
            (Species::Neutrophil, 5, 5., 1.),
            (Species::MotileBacterium, 5, 7., 1.),
            (Species::MotileBacterium, 5, 10., -1.),
        ];
        for (species, edge, along, side) in encounters {
            let mut found = None;
            for offset in 0..80 {
                let step = offset as f32 * 0.25;
                for delta in [step, -step] {
                    let at = along + delta;
                    if at < 1. || at > sim.world_map().unwrap().edge_length(edge) - 1. {
                        continue;
                    }
                    if let Some((point, _)) = sim.spawn_candidate(edge, at, side, species) {
                        found = Some(point);
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
            sim.insert_species(
                found.expect("small_world combat encounter has legal space"),
                species,
            );
        }
        sim
    }

    pub(super) fn update_white_intents(&mut self) {
        self.immunity.advance(self.config.dt);
        let facts = self.active_facts();
        for cell in &mut self.cells {
            let Some(handle) = cell.active_body() else {
                continue;
            };
            let species = cell.species;
            let id = cell.id;
            let dead = cell.dead_age.is_some();
            let Some(brain) = cell.behavior.as_mut() else {
                continue;
            };
            if dead {
                brain.movement = Vector::ZERO;
                brain.contact_time = 0.;
                brain.call_time = 0.;
                continue;
            }
            let center = fluid::center(&self.physics.soft_bodies[handle]);
            let radius = if species.is_immune() {
                self.immunity.sense_radius()
            } else {
                4.
            };
            let visible: Vec<_> = facts
                .iter()
                .filter(|f| {
                    f.target != TargetRef::Cell(id)
                        && center.distance(f.position) <= radius
                        && crate::combat::clear_path(&self.physics, center, f.position)
                })
                .collect();
            let legal = |f: &&Seen| {
                if species.is_immune() {
                    f.species.is_some_and(Species::is_pathogen)
                        || (f.target == TargetRef::Robot
                            && (self.immunity.alertness >= WANTED_THRESHOLD
                                || brain.target == Some(TargetRef::Robot)
                                || brain.attacker == Some(TargetRef::Robot)))
                } else {
                    f.species == Some(Species::RedCell)
                        || (Some(f.target) == brain.attacker
                            && f.species.is_some_and(Species::is_immune))
                }
            };
            let chosen = visible
                .iter()
                .copied()
                .filter(legal)
                .find(|f| Some(f.target) == brain.attacker)
                .or_else(|| {
                    visible
                        .iter()
                        .copied()
                        .filter(legal)
                        .find(|f| Some(f.target) == brain.target)
                })
                .or_else(|| {
                    visible.iter().copied().filter(legal).min_by(|a, b| {
                        center
                            .distance_squared(a.position)
                            .total_cmp(&center.distance_squared(b.position))
                            .then_with(|| target_id(a.target).cmp(&target_id(b.target)))
                    })
                });
            if !species.is_immune() && chosen.is_none() {
                brain.goal = None;
                brain.set_target(None);
            }
            // A removed/dead target cannot retain an actionable identity.
            if brain
                .target
                .is_some_and(|t| !facts.iter().any(|f| f.target == t))
            {
                brain.set_target(None);
            }
            let options = species.options(&self.config);
            brain.think(
                center,
                chosen.map(|f| (f.target, f.position)),
                self.immunity.alertness,
                self.config.dt,
                &options,
            );
            let mut desired = brain.movement;
            if species == Species::Macrophage && brain.contact_time > 0. {
                desired *= 0.5;
            }
            brain.movement = local_steering(&self.physics, center, desired, options.radius, id);
            if desired != Vector::ZERO && brain.movement == Vector::ZERO {
                let side = if id % 2 == 0 { 1.0 } else { -1.0 };
                brain.heading = crate::white_cell::rotate(
                    brain.heading,
                    side * 2.0 * std::f32::consts::PI * self.config.dt / options.turn_time,
                );
            }
        }
    }

    pub(super) fn react_to_hits(&mut self, hits: &[crate::combat::Hit]) {
        // Only actual robot damage to normal/immune cells changes wanted level.
        let increase: f32 = hits
            .iter()
            .filter(|h| h.source == TargetRef::Robot && !h.species.is_pathogen())
            .map(|h| {
                h.damage * if h.species.is_immune() { 0.12 } else { 0.04 }
                    + if h.killed {
                        if h.species.is_immune() { 10. } else { 4. }
                    } else {
                        0.
                    }
            })
            .sum();
        if increase > 0. {
            self.immunity.alertness = (self.immunity.alertness + increase).min(100.);
            self.immunity.quiet_time = 0.;
        }
        let facts = self.active_facts();
        for cell in &mut self.cells {
            let Some(handle) = cell.active_body() else {
                continue;
            };
            if cell.dead_age.is_some() {
                continue;
            }
            let id = cell.id;
            let species = cell.species;
            let Some(brain) = cell.behavior.as_mut() else {
                continue;
            };
            let center = fluid::center(&self.physics.soft_bodies[handle]);
            let radius = if species.is_immune() {
                self.immunity.sense_radius()
            } else {
                4.
            };
            let sees = |point: Vector| {
                center.distance(point) <= radius
                    && crate::combat::clear_path(&self.physics, center, point)
            };
            let relevant = |h: &&crate::combat::Hit| {
                if !facts.iter().any(|f| f.target == h.source) {
                    return false;
                }
                if species.is_immune() {
                    // Clearing pathogens never identifies the robot as an offender.
                    !h.species.is_pathogen()
                        && (h.source == TargetRef::Robot
                            || facts.iter().any(|f| {
                                f.target == h.source && f.species.is_some_and(Species::is_pathogen)
                            }))
                } else {
                    h.id == id
                        && facts.iter().any(|f| {
                            f.target == h.source && f.species.is_some_and(Species::is_immune)
                        })
                }
            };
            let direct = hits
                .iter()
                .filter(relevant)
                .filter(|h| h.id == id)
                .max_by(|a, b| {
                    a.damage
                        .total_cmp(&b.damage)
                        .then_with(|| target_id(b.source).cmp(&target_id(a.source)))
                });
            let witnessed = hits
                .iter()
                .filter(relevant)
                .filter(|h| sees(h.position) && sees(h.source_position))
                .max_by(|a, b| {
                    a.damage
                        .total_cmp(&b.damage)
                        .then_with(|| target_id(b.source).cmp(&target_id(a.source)))
                });
            let keep_target = brain
                .target
                .is_some_and(|t| facts.iter().any(|f| f.target == t && sees(f.position)));
            if let Some(hit) = direct.or(witnessed.filter(|_| !keep_target)) {
                if direct.is_some() {
                    brain.attacker = Some(hit.source);
                }
                if hit.source == TargetRef::Robot {
                    brain.pursue(hit.source_position, self.immunity.alertness);
                } else {
                    brain.set_target(Some(hit.source));
                    brain.goal = Some(Goal::Pursue(hit.source_position));
                    brain.memory = 2. + 3. * self.immunity.alertness / 100.;
                }
            } else if species.is_immune() && !brain.is_pursuing() {
                if let Some(hit) = hits.iter().filter(relevant).find(|h| sees(h.position)) {
                    brain.goal = Some(Goal::Investigate(hit.position));
                    brain.memory = 3.;
                }
            }
        }
    }

    pub(super) fn drive_white_cells(&mut self, dt: f32) {
        for index in 0..self.cells.len() {
            let cell = &self.cells[index];
            let Some(brain) = cell.behavior.as_ref() else {
                continue;
            };
            if cell.dead_age.is_some() {
                continue;
            }
            let Some(handle) = cell.active_body() else {
                continue;
            };
            let body = &self.physics.soft_bodies[handle];
            let center = fluid::center(body);
            let target = self.flow_velocity_at(center, fluid::body_radius(body)) + brain.movement;
            let body = &mut self.physics.soft_bodies[handle];
            let mean = body.particle_velocities().sum::<Vector>() / body.num_particles() as f32;
            let delta = (target - mean) * (1. - (-30. * dt).exp());
            for node in 0..body.num_particles() {
                body.set_particle_velocity(node, body.particle_velocity(node) + delta);
            }
        }
    }

    pub(super) fn finish_white_actions(&mut self) {
        let facts = self.active_facts();
        let contacts: Vec<_> = self
            .cells
            .iter()
            .filter_map(|c| Some((c.id, c.active_body()?)))
            .collect();
        let mut actions = Vec::new();
        let mut call = None;
        for cell in &mut self.cells {
            if cell.dead_age.is_some() {
                continue;
            }
            let Some(handle) = cell.active_body() else {
                continue;
            };
            let species = cell.species;
            let id = cell.id;
            let Some(brain) = cell.behavior.as_mut() else {
                continue;
            };
            let body = &self.physics.soft_bodies[handle];
            let center = fluid::center(body);
            let target = brain
                .target
                .and_then(|t| facts.iter().find(|f| f.target == t));
            let radius = if species.is_immune() {
                self.immunity.sense_radius()
            } else {
                4.
            };
            let visible = target.filter(|f| {
                center.distance(f.position) <= radius
                    && crate::combat::clear_path(&self.physics, center, f.position)
            });
            let contact = visible.is_some_and(|f| match f.target {
                TargetRef::Robot => crate::combat::contact_point(body, f.position, 1.05).is_some(),
                TargetRef::Cell(other) => {
                    contacts
                        .iter()
                        .find(|(id, _)| *id == other)
                        .is_some_and(|(_, h)| {
                            crate::combat::contours_touch(body, &self.physics.soft_bodies[*h])
                        })
                }
            });
            let options = species.options(&self.config);
            if contact {
                brain.contact_time += self.config.dt;
                if brain.contact_time + 1e-6 >= options.contact_warmup
                    && brain.attack_cooldown <= 1e-6
                {
                    let target = visible.unwrap();
                    let damage = options.damage
                        * if species.is_immune()
                            && target.species == Some(Species::EncapsulatedBacterium)
                        {
                            0.5
                        } else {
                            1.
                        };
                    actions.push((TargetRef::Cell(id), center, target.target, damage));
                    brain.attack_cooldown = options.attack_interval;
                }
            } else {
                brain.contact_time = 0.;
            }
            if species.is_immune()
                && visible.is_some_and(|f| f.target == TargetRef::Robot)
                && self.immunity.alertness >= WANTED_THRESHOLD
                && self.immunity.call_cooldown <= 1e-6
            {
                brain.call_time += self.config.dt;
                if brain.call_time + 1e-6 >= self.config.white_cells.call_time && call.is_none() {
                    call = Some((center, visible.unwrap().position));
                }
            } else {
                brain.call_time = 0.;
            }
        }
        // All actions above came from alive bodies. Apply the batch before reactions,
        // so mutually lethal hits do not depend on the order of the cell vector.
        let mut damaged = Vec::new();
        for cell in &mut self.cells {
            let amount: f32 = actions
                .iter()
                .filter(|a| a.2 == TargetRef::Cell(cell.id))
                .map(|a| a.3)
                .sum();
            if amount == 0. {
                continue;
            }
            let previous = cell.health;
            cell.damage(amount);
            damaged.push((
                cell.id,
                cell.species,
                fluid::center(&self.physics.soft_bodies[cell.active_body().unwrap()]),
                previous - cell.health,
                cell.dead_age.is_some(),
            ));
        }
        let alive = self.active_facts();
        let hits: Vec<_> = damaged
            .into_iter()
            .filter_map(|(id, species, position, damage, killed)| {
                let source = actions
                    .iter()
                    .filter(|a| a.2 == TargetRef::Cell(id) && alive.iter().any(|f| f.target == a.0))
                    .max_by(|a, b| {
                        a.3.total_cmp(&b.3)
                            .then_with(|| target_id(b.0).cmp(&target_id(a.0)))
                    })?;
                Some(crate::combat::Hit {
                    id,
                    species,
                    position,
                    damage,
                    killed,
                    source: source.0,
                    source_position: source.1,
                })
            })
            .collect();
        self.robot_core.damage(
            actions
                .iter()
                .filter(|a| a.2 == TargetRef::Robot)
                .map(|a| a.3)
                .sum(),
        );
        self.react_to_hits(&hits);
        if self.robot_core.health == 0. {
            self.paused = true;
            return;
        }
        if let Some((caller, last_seen)) = call {
            self.try_reinforcement(caller, last_seen);
        }
    }

    pub(super) fn try_reinforcement(&mut self, caller: Vector, last_seen: Vector) {
        self.immunity.call_cooldown = self.config.white_cells.call_interval;
        for cell in &mut self.cells {
            if let Some(b) = cell.behavior.as_mut() {
                b.call_time = 0.;
            }
        }
        if self.cells.iter().filter(|c| c.species.is_immune()).count()
            >= self.config.white_cells.max_bodies
        {
            return;
        }
        let species = if self.combat_preview
            && self.immunity.alertness >= 80.
            && !self.cells.iter().any(|c| c.species == Species::Macrophage)
        {
            Species::Macrophage
        } else {
            Species::Neutrophil
        };
        if let Some((position, wall)) = self.reinforcement_position(caller, species) {
            let index = self.insert_species(position, species);
            let b = self.cells[index].behavior.as_mut().unwrap();
            b.goal = Some(Goal::Investigate(last_seen));
            b.memory = 3.;
            b.arrival_time = ARRIVAL_TIME;
            self.immunity.arrival = Some(Arrival {
                wall,
                position,
                remaining: ARRIVAL_TIME,
            });
        }
    }

    pub(super) fn insert_white_cell(&mut self, position: Vector) -> usize {
        self.insert_species(position, Species::Neutrophil)
    }

    pub(super) fn insert_species(&mut self, position: Vector, species: Species) -> usize {
        if species.is_immune() {
            self.white_cells_enabled = true;
        }
        let options = species.options(&self.config);
        let body = if self.streaming.as_ref().is_some_and(|s| {
            !s.center
                .neighborhood(crate::map::SCENE_RADIUS)
                .contains(&crate::map::ChunkId::at(position.to_array()))
        }) {
            cell::CellBody::Dormant(cell::DormantBody::at(
                position,
                options.radius,
                &self.config,
            ))
        } else {
            cell::CellBody::Active(cell::insert_with_radius(
                &mut self.physics,
                position,
                options.radius,
                &self.config,
            ))
        };
        let id = self.next_cell_id;
        self.next_cell_id += 1;
        let mut cell = cell::Cell::new(id, body, options.health, cell::UnloadPolicy::Hibernate);
        cell.species = species;
        cell.behavior = if species.is_immune() {
            crate::pathogen::Behavior::Immune(WhiteCell::new(id))
        } else {
            crate::pathogen::Behavior::Bacterium(WhiteCell::new(id))
        };
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
                        if let Some((position, _)) =
                            self.spawn_candidate(edge, along, side, Species::Neutrophil)
                        {
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

    fn reinforcement_position(&self, caller: Vector, species: Species) -> Option<(Vector, Vector)> {
        let map = self.world_map()?;
        let edge = map.nearest_edge(caller);
        let along = map.project_edge(edge, caller).along;
        for offset in [3.0, -3.0, 5.0, -5.0, 7.0, -7.0] {
            let candidate = along + offset;
            if candidate < 1.5 || candidate > map.edge_length(edge) - 1.5 {
                continue;
            }
            for side in [-1.0, 1.0] {
                if let Some(point) = self.spawn_candidate(edge, candidate, side, species) {
                    return Some(point);
                }
            }
        }
        None
    }

    fn spawn_candidate(
        &self,
        edge: usize,
        along: f32,
        side: f32,
        species: Species,
    ) -> Option<(Vector, Vector)> {
        let map = self.world_map()?;
        let radius = species.radius(&self.config);
        let half_width = map.edges[edge].half_width;
        if half_width <= radius + 0.25 {
            return None;
        }
        let position = map.edge_point(edge, along, side * (half_width - radius - 0.3));
        if map.signed_distance(position) > -radius - 0.25
            || map.nearest_edge(position) != edge
            || position.distance(self.physics.bodies[self.robot].translation()) < radius + 1.35
            || self.cells.iter().any(|cell| {
                let Some(handle) = cell.active_body() else {
                    return false;
                };
                let (min, max) = cell_bounds(&self.physics.soft_bodies[handle]);
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

fn target_id(target: TargetRef) -> u64 {
    match target {
        TargetRef::Robot => u64::MAX,
        TargetRef::Cell(id) => id,
    }
}

fn local_steering(
    physics: &PhysicsWorld,
    center: Vector,
    desired: Vector,
    radius: f32,
    id: u64,
) -> Vector {
    use rapier2d::parry::{query::ShapeCastOptions, shape::Ball};
    if desired == Vector::ZERO {
        return desired;
    }
    let query = physics.query_pipeline_with_filter(QueryFilter::only_fixed());
    let direction = desired.normalize();
    let side = if id % 2 == 0 { 1. } else { -1. };
    for angle in [
        0.,
        side * std::f32::consts::FRAC_PI_4,
        -side * std::f32::consts::FRAC_PI_4,
    ] {
        let d = crate::white_cell::rotate(direction, angle);
        // Shape cast covers only the immediate 0.6-unit step; the ball represents body radius.
        if query
            .cast_shape(
                &Pose::from_translation(center),
                d,
                &Ball::new(radius),
                ShapeCastOptions {
                    max_time_of_impact: 0.6,
                    ..Default::default()
                },
            )
            .is_none()
        {
            return d * desired.length();
        }
    }
    Vector::ZERO
}
