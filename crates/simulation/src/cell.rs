use crate::Config;
use rapier2d::prelude::*;

pub(crate) fn insert(world: &mut PhysicsWorld, center: Vector, config: &Config) -> SoftBodyHandle {
    world.insert_soft_body(
        SoftBodyBuilder::disk(center, config.cell_radius, config.cell_vertices)
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
    // Velocity relaxation integrates drag analytically. Contacts and deformation
    // still run in Rapier; positions are never moved directly by the flow.
    let response = 1.0 - (-drag * dt).exp();
    for i in 0..body.num_particles() {
        let target = velocity_at(body.particle_position(i));
        let velocity = body.particle_velocity(i);
        body.set_particle_velocity(i, velocity + (target - velocity) * response);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
