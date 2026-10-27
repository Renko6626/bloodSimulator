use crate::{Config, MoveInput};
use rapier2d::prelude::*;

pub(crate) fn insert(world: &mut PhysicsWorld, center: Vector) -> RigidBodyHandle {
    world
        .insert(
            RigidBodyBuilder::dynamic()
                .translation(center)
                .lock_rotations()
                .ccd_enabled(true),
            ColliderBuilder::ball(0.85).mass(3.0).friction(0.15),
        )
        .0
}

pub(crate) fn drive(
    body: &mut RigidBody,
    input: MoveInput,
    flow_velocity: Vector,
    config: &Config,
) {
    let movement = Vector::new(input.x, input.y).clamp_length_max(1.0);
    let target = movement * config.robot_speed + flow_velocity * config.robot_flow;
    // Overdamped control: contacts still resolve in Rapier, without input coasting.
    body.set_linvel(target, true);
}
