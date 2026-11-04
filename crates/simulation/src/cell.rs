use crate::Config;
use rapier2d::prelude::*;

/// How an entity should be represented after leaving detailed simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnloadPolicy {
    /// Return ordinary live cells to the population stock, discarding identity.
    Aggregate,
    /// Preserve individual state while its detailed body is unloaded.
    Hibernate,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DormantBody {
    pub positions: Vec<Vector>,
    pub velocities: Vec<Vector>,
}

impl DormantBody {
    pub fn at(center: Vector, species: crate::Species, id: u64, config: &Config) -> Self {
        Self {
            positions: species_builder(center, species, id, config)
                .particle_positions()
                .to_vec(),
            velocities: vec![Vector::ZERO; config.cell_vertices],
        }
    }
    pub fn center(&self) -> Vector {
        self.positions.iter().copied().sum::<Vector>() / self.positions.len() as f32
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CellBody {
    Active(SoftBodyHandle),
    Dormant(DormantBody),
}

impl From<SoftBodyHandle> for CellBody {
    fn from(handle: SoftBodyHandle) -> Self {
        Self::Active(handle)
    }
}

/// Biological state is independent of the Rapier body and its current shape.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cell {
    pub core: crate::entity::EntityCore,
    pub body: CellBody,
    pub unload_policy: UnloadPolicy,
    pub species: crate::Species,
    pub behavior: crate::pathogen::Behavior,
}

impl std::ops::Deref for Cell {
    type Target = crate::entity::EntityCore;
    fn deref(&self) -> &Self::Target {
        &self.core
    }
}
impl std::ops::DerefMut for Cell {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}
impl Cell {
    pub fn new(
        id: u64,
        body: impl Into<CellBody>,
        health: f32,
        unload_policy: UnloadPolicy,
    ) -> Self {
        Self {
            core: crate::entity::EntityCore::new(id, health),
            body: body.into(),
            unload_policy,
            species: crate::Species::RedCell,
            behavior: crate::pathogen::Behavior::Passive,
        }
    }

    pub fn active_body(&self) -> Option<SoftBodyHandle> {
        match self.body {
            CellBody::Active(handle) => Some(handle),
            CellBody::Dormant(_) => None,
        }
    }

    pub fn hibernate(&mut self, physics: &mut PhysicsWorld) {
        let Some(handle) = self.active_body() else {
            return;
        };
        let body = &physics.soft_bodies[handle];
        let state = DormantBody {
            positions: body.particle_positions().collect(),
            velocities: body.particle_velocities().collect(),
        };
        physics.remove_soft_body(handle);
        self.body = CellBody::Dormant(state);
    }

    pub fn restore(&mut self, physics: &mut PhysicsWorld, config: &Config) {
        let CellBody::Dormant(state) = &self.body else {
            return;
        };
        let handle = insert_species(physics, state.center(), self.species, self.id, config);
        let body = &mut physics.soft_bodies[handle];
        for (i, (&position, &velocity)) in state.positions.iter().zip(&state.velocities).enumerate()
        {
            body.set_particle_position(i, position);
            body.set_particle_velocity(i, velocity);
        }
        self.apply_corpse_material(body, config);
        self.body = CellBody::Active(handle);
        if let Some(brain) = self.behavior.as_mut() {
            brain.contact_time = 0.0;
        }
    }

    /// Only change constitutive parameters; Rapier resolves the shrinking area.
    pub fn advance_corpse(&mut self, physics: &mut PhysicsWorld, config: &Config) -> bool {
        let Some(age) = &mut self.dead_age else {
            return false;
        };
        *age += config.dt;
        if *age >= config.corpse_lifetime {
            return true;
        }
        if let Some(handle) = self.active_body() {
            self.apply_corpse_material(&mut physics.soft_bodies[handle], config);
        }
        false
    }

    fn apply_corpse_material(&self, body: &mut SoftBody, config: &Config) {
        let Some(age) = self.dead_age else { return };
        let t = (age / config.corpse_soften_time).min(1.0);
        let material = body.material_mut();
        material.edge_softness.natural_frequency =
            config.cell_frequency + (config.corpse_edge_frequency - config.cell_frequency) * t;
        material.bend_softness.natural_frequency =
            config.cell_frequency + (config.corpse_bend_frequency - config.cell_frequency) * t;
        body.set_volume_factor(1.0 + (config.corpse_area_factor - 1.0) * t);
    }

    pub fn opacity(&self, config: &Config) -> f32 {
        self.dead_age.map_or(1.0, |age| {
            ((config.corpse_lifetime - age) / 0.5).clamp(0.0, 1.0)
        })
    }
}

pub(crate) fn insert(world: &mut PhysicsWorld, center: Vector, config: &Config) -> SoftBodyHandle {
    insert_with_radius(world, center, config.cell_radius, config)
}

pub(crate) fn insert_with_radius(
    world: &mut PhysicsWorld,
    center: Vector,
    radius: f32,
    config: &Config,
) -> SoftBodyHandle {
    insert_builder(
        world,
        SoftBodyBuilder::disk(center, radius, config.cell_vertices),
        config,
    )
}

pub(crate) fn insert_species(
    world: &mut PhysicsWorld,
    center: Vector,
    species: crate::Species,
    id: u64,
    config: &Config,
) -> SoftBodyHandle {
    insert_builder(world, species_builder(center, species, id, config), config)
}

fn species_builder(
    center: Vector,
    species: crate::Species,
    id: u64,
    config: &Config,
) -> SoftBodyBuilder {
    let radius = species.radius(config);
    let disk = SoftBodyBuilder::disk(center, radius, config.cell_vertices);
    if !species.is_immune() {
        return disk;
    }
    // Broad, low-amplitude lobes rather than independent noisy vertices.
    // Build constraints FROM these points: moving disk particles later would
    // leave circular rest lengths in place. Identity also reproduces this on wake.
    let phase = (id % 4096) as f32 * 2.399_963_1;
    let n = disk.particle_positions().len();
    let mut points: Vec<_> = (0..n)
        .map(|i| {
            let angle = std::f32::consts::TAU * i as f32 / n as f32;
            let radial = 1.0
                + 0.07 * (2.0 * angle + phase).cos()
                + 0.03 * (3.0 * angle - phase * 0.73).cos();
            Vector::new(angle.cos(), angle.sin()) * radial
        })
        .collect();
    let mean = points.iter().copied().sum::<Vector>() / n as f32;
    for p in &mut points {
        *p -= mean;
    }
    let area = (0..n)
        .map(|i| points[i].perp_dot(points[(i + 1) % n]))
        .sum::<f32>()
        * 0.5;
    let disk_area = 0.5 * n as f32 * radius * radius * (std::f32::consts::TAU / n as f32).sin();
    let scale = (disk_area / area).sqrt();
    for p in &mut points {
        *p = center + *p * scale;
    }
    SoftBodyBuilder::polygon(points)
}

fn insert_builder(
    world: &mut PhysicsWorld,
    builder: SoftBodyBuilder,
    config: &Config,
) -> SoftBodyHandle {
    world.insert_soft_body(
        builder
            .mass(1.0)
            .material(SoftBodyMaterial {
                volume_softness: SpringCoefficients::new(config.cell_area_frequency, 1.0),
                ..SoftBodyMaterial::uniform(SpringCoefficients::new(config.cell_frequency, 1.0))
            })
            .linear_damping(config.cell_damping)
            .particle_radius(0.07)
            .surface_collider(ColliderBuilder::ball(0.07).friction(0.12))
            .can_sleep(false),
    )
}

pub(crate) fn advect(
    body: &mut SoftBody,
    drag: f32,
    dt: f32,
    velocity_at: impl Fn(Vector) -> Vector,
) {
    if drag == 0.0 {
        // Rapier setters also clear a pending kinematic position; retain that
        // bookkeeping while omitting queries whose response would be zero.
        for i in 0..body.num_particles() {
            body.set_particle_velocity(i, body.particle_velocity(i));
        }
        return;
    }
    // Velocity relaxation integrates drag analytically. Contacts and deformation
    // still run in Rapier; positions are never moved directly by the flow.
    let response = 1.0 - (-drag * dt).exp();
    if body.num_particles() == 12 {
        let samples: [Vector; 6] =
            std::array::from_fn(|k| velocity_at(body.particle_position(k * 2)));
        for i in 0..12 {
            let k = i / 2;
            let target = if i % 2 == 0 {
                samples[k]
            } else {
                (samples[k] + samples[(k + 1) % 6]) * 0.5
            };
            let velocity = body.particle_velocity(i);
            body.set_particle_velocity(i, velocity + (target - velocity) * response);
        }
        return;
    }
    for i in 0..body.num_particles() {
        let target = velocity_at(body.particle_position(i));
        let velocity = body.particle_velocity(i);
        body.set_particle_velocity(i, velocity + (target - velocity) * response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hibernate_restores_identity_health_and_deformed_body() {
        let config = Config::default();
        let mut physics = PhysicsWorld::new();
        let body = insert(&mut physics, Vector::ZERO, &config);
        let rest_area = physics.soft_bodies[body].volume();
        for i in 0..config.cell_vertices {
            let p = physics.soft_bodies[body].particle_position(i);
            physics.soft_bodies[body].set_particle_position(i, Vector::new(p.x * 1.25, p.y * 0.8));
            physics.soft_bodies[body].set_particle_velocity(i, Vector::X);
        }
        let shape: Vec<_> = physics.soft_bodies[body].particle_positions().collect();
        let mut cell = Cell::new(17, body, 80., UnloadPolicy::Hibernate);
        cell.damage(30.);
        cell.hibernate(&mut physics);
        assert!(physics.soft_bodies.get(body).is_none());
        assert!(cell.active_body().is_none());
        cell.restore(&mut physics, &config);
        let restored = &physics.soft_bodies[cell.active_body().unwrap()];
        assert_eq!(cell.id, 17);
        assert_eq!(cell.health, 50.);
        assert_eq!(restored.particle_positions().collect::<Vec<_>>(), shape);
        assert!(restored.particle_velocities().all(|v| v == Vector::X));
        assert!((restored.volume() - rest_area).abs() < 1e-4);
    }

    #[test]
    fn immune_cells_keep_individual_rest_shapes_after_deformation_and_wake() {
        let config = Config::default();
        for species in [crate::Species::Neutrophil, crate::Species::Macrophage] {
            let mut world = PhysicsWorld::new();
            world.gravity = Vector::ZERO;
            world.integration_parameters.dt = config.dt / config.substeps as f32;
            let center = Vector::new(3.0, -2.0);
            let handle = insert_species(&mut world, center, species, 82, &config);
            let initial: Vec<_> = world.soft_bodies[handle].particle_positions().collect();
            let radius = species.radius(&config);
            let radial: Vec<_> = initial.iter().map(|p| p.distance(center)).collect();
            let min = radial.iter().copied().fold(f32::INFINITY, f32::min);
            let max = radial.iter().copied().fold(0.0_f32, f32::max);
            assert!(max - min > 0.1 * radius);
            assert!(max < 1.12 * radius);
            let dormant = DormantBody::at(center, species, 82, &config);
            assert_eq!(dormant.positions, initial);
            assert_ne!(
                DormantBody::at(center, species, 83, &config).positions,
                initial
            );
            let area = measurements(&world.soft_bodies[handle]).0;
            let regular_area = 0.5
                * config.cell_vertices as f32
                * radius.powi(2)
                * (std::f32::consts::TAU / config.cell_vertices as f32).sin();
            assert!((area / regular_area - 1.0).abs() < 1e-5);
            for _ in 0..120 {
                world.step();
            }
            for (i, p) in initial.iter().enumerate() {
                assert!(world.soft_bodies[handle].particle_position(i).distance(*p) < 0.01);
                let delta = *p - center;
                world.soft_bodies[handle]
                    .set_particle_position(i, center + Vector::new(delta.x * 1.2, delta.y / 1.2));
            }
            let mut cell = Cell::new(82, handle, 100.0, UnloadPolicy::Hibernate);
            cell.species = species;
            cell.hibernate(&mut world);
            cell.restore(&mut world, &config);
            for _ in 0..480 {
                world.step();
            }
            let body = &world.soft_bodies[cell.active_body().unwrap()];
            // Pairwise distances ignore any harmless rigid translation/rotation.
            for i in 0..initial.len() {
                for j in i + 1..initial.len() {
                    let recovered = body
                        .particle_position(i)
                        .distance(body.particle_position(j));
                    assert!(
                        (recovered - initial[i].distance(initial[j])).abs() < 0.025 * radius,
                        "{species:?}: pair {i}/{j} did not recover"
                    );
                }
            }
        }
    }

    fn measurements(body: &SoftBody) -> (f32, f32, f32) {
        let points: Vec<_> = body.particle_positions().collect();
        assert!(points.iter().all(|p| p.is_finite()));
        let area = (0..points.len())
            .map(|i| points[i].perp_dot(points[(i + 1) % points.len()]))
            .sum::<f32>()
            .abs()
            * 0.5;
        let min = points
            .iter()
            .fold(Vector::splat(f32::INFINITY), |a, &p| a.min(p));
        let max = points
            .iter()
            .fold(Vector::splat(f32::NEG_INFINITY), |a, &p| a.max(p));
        (area, max.x - min.x, max.y - min.y)
    }

    #[test]
    fn compressed_cell_preserves_area_and_recovers_after_release() {
        let config = Config::default();
        let mut world = PhysicsWorld::new();
        world.gravity = Vector::ZERO;
        world.integration_parameters.dt = config.dt / config.substeps as f32;
        let cell = insert(&mut world, Vector::ZERO, &config);
        let top = world
            .insert(
                RigidBodyBuilder::kinematic_position_based().translation(Vector::new(0.0, -2.0)),
                ColliderBuilder::cuboid(4.0, 0.15),
            )
            .0;
        let bottom = world
            .insert(
                RigidBodyBuilder::kinematic_position_based().translation(Vector::new(0.0, 2.0)),
                ColliderBuilder::cuboid(4.0, 0.15),
            )
            .0;
        let initial = measurements(&world.soft_bodies[cell]);
        for i in 0..240 {
            let gap = 2.0 - 1.35 * (i as f32 / 120.0).min(1.0);
            world.bodies[top].set_next_kinematic_translation(Vector::new(0.0, -gap));
            world.bodies[bottom].set_next_kinematic_translation(Vector::new(0.0, gap));
            world.step();
        }
        let compressed = measurements(&world.soft_bodies[cell]);
        for i in 0..360 {
            let gap = 0.65 + 1.35 * (i as f32 / 120.0).min(1.0);
            world.bodies[top].set_next_kinematic_translation(Vector::new(0.0, -gap));
            world.bodies[bottom].set_next_kinematic_translation(Vector::new(0.0, gap));
            world.step();
        }
        let recovered = measurements(&world.soft_bodies[cell]);
        println!(
            "area/width/height: initial={initial:?}, compressed={compressed:?}, recovered={recovered:?}"
        );
        // Broad prototype acceptance: visible flattening, no area collapse,
        // and recovery within 20% of the initial bounding dimensions.
        assert!(compressed.2 < initial.2 * 0.9);
        assert!((compressed.0 / initial.0 - 1.0).abs() < 0.2);
        assert!((recovered.0 / initial.0 - 1.0).abs() < 0.2);
        assert!((recovered.1 / initial.1 - 1.0).abs() < 0.2);
        assert!((recovered.2 / initial.2 - 1.0).abs() < 0.2);
    }
}
