//! One equivalent cylindrical vessel, driven by a lumped pump pressure difference.
pub mod graph;

use crate::vessel::Vessel;
use rapier2d::prelude::Vector;
use std::f32::consts::PI;

pub struct Network {
    resistance: f32,
    cross_section: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Flow {
    pub pressure_drop: f32,
    pub resistance: f32,
    pub rate: f32,
    pub mean_speed: f32,
}

impl Network {
    pub fn new(vessel: &Vessel, viscosity: f32) -> Self {
        assert!(viscosity.is_finite() && viscosity > 0.0);
        Self {
            resistance: 8.0 * viscosity * vessel.length() / (PI * vessel.half_width.powi(4)),
            cross_section: PI * vessel.half_width.powi(2),
        }
    }

    pub fn solve(&self, pressure_drop: f32) -> Flow {
        let rate = pressure_drop / self.resistance;
        Flow {
            pressure_drop,
            resistance: self.resistance,
            rate,
            mean_speed: rate / self.cross_section,
        }
    }
}

/// Circular-pipe Poiseuille profile sampled on a 2D central section.
/// The bends use the local tangent; this is not a curved-pipe fluid solver.
pub fn sample_velocity(position: Vector, vessel: &Vessel, flow: Flow) -> Vector {
    let projection = vessel.project(position);
    let profile = (1.0 - (projection.offset / vessel.half_width).powi(2)).max(0.0);
    projection.tangent * (2.0 * flow.mean_speed * profile)
}
