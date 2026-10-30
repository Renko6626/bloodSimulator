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
    pub fn at(center: Vector, radius: f32, config: &Config) -> Self {
        Self {
            positions: SoftBodyBuilder::disk(center, radius, config.cell_vertices)
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
    pub id: u64,
    pub body: CellBody,
    pub unload_policy: UnloadPolicy,
    pub health: f32,
    pub max_health: f32,
    pub dead_age: Option<f32>,
    /// Only white cells carry active behavior. Ordinary red cells remain passive.
    pub white: Option<crate::white_cell::WhiteCell>,
}

impl Cell {
    pub fn new(
        id: u64,
        body: impl Into<CellBody>,
        health: f32,
        unload_policy: UnloadPolicy,
    ) -> Self {
        Self {
            id,
            body: body.into(),
            unload_policy,
            health,
            max_health: health,
            dead_age: None,
            white: None,
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
        let radius = if self.white.is_some() {
            config.white_cells.radius
        } else {
            config.cell_radius
        };
        let handle = insert_with_radius(physics, state.center(), radius, config);
        let body = &mut physics.soft_bodies[handle];
        for (i, (&position, &velocity)) in state.positions.iter().zip(&state.velocities).enumerate()
        {
            body.set_particle_position(i, position);
            body.set_particle_velocity(i, velocity);
        }
        self.apply_corpse_material(body, config);
        self.body = CellBody::Active(handle);
    }

    pub fn damage(&mut self, amount: f32) {
        if self.dead_age.is_some() {
            return;
        }
        self.health = (self.health - amount).max(0.0);
        if self.health == 0.0 {
            self.dead_age = Some(0.0);
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
    world.insert_soft_body(
        SoftBodyBuilder::disk(center, radius, config.cell_vertices)
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
