use crate::Config;
use rapier2d::{parry::query::PointQuery, prelude::*};

pub(crate) const ATTACK_FLASH_TIME: f32 = 0.12;

pub(crate) struct Hit {
    pub id: u64,
    pub position: Vector,
    pub white: bool,
    pub damage: f32,
    pub killed: bool,
}

#[derive(Default)]
pub(crate) struct Attack {
    pub held: bool,
    pub cooldown: f32,
    pub flash_time: f32,
    pub center: Vector,
}

impl Attack {
    pub fn advance(&mut self, center: Vector, config: &Config) -> bool {
        self.cooldown = (self.cooldown - config.dt).max(0.0);
        self.flash_time = (self.flash_time - config.dt).max(0.0);
        if !self.held || self.cooldown > 0.0 {
            return false;
        }
        self.cooldown = config.attack_interval;
        self.flash_time = ATTACK_FLASH_TIME;
        self.center = center;
        true
    }
}

pub(crate) fn clear_path(world: &PhysicsWorld, from: Vector, to: Vector) -> bool {
    let direction = to - from;
    direction.length_squared() < 1.0e-8
        || world
            .query_pipeline_with_filter(QueryFilter::only_fixed())
            .cast_ray(&Ray::new(from, direction), 1.0, true)
            .is_none()
}

/// Nearest point on the actual filled cell contour, not its undeformed radius.
pub(crate) fn contact_point(body: &SoftBody, origin: Vector, radius: f32) -> Option<Vector> {
    let points: Vec<_> = body.particle_positions().collect();
    if rapier2d::parry::utils::point_in_poly2d(origin, &points) {
        return Some(origin);
    }
    let closest = (0..points.len())
        .map(|i| {
            rapier2d::parry::shape::Segment::new(points[i], points[(i + 1) % points.len()])
                .project_local_point(origin, true)
                .point
        })
        .min_by(|a, b| {
            a.distance_squared(origin)
                .total_cmp(&b.distance_squared(origin))
        })?;
    (closest.distance_squared(origin) <= radius * radius).then_some(closest)
}
