//! A closed, constant-width racetrack. Arc length increases clockwise in +Y-down space.
use rapier2d::prelude::Vector;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

#[derive(Clone, Debug)]
pub struct Vessel {
    pub straight_length: f32,
    pub bend_radius: f32,
    pub half_width: f32,
}

pub struct Projection {
    pub distance: f32,
    pub offset: f32,
    pub tangent: Vector,
}

impl Default for Vessel {
    fn default() -> Self {
        Self {
            straight_length: 20.0,
            bend_radius: 6.0,
            half_width: 2.2,
        }
    }
}

impl Vessel {
    pub fn length(&self) -> f32 {
        2.0 * self.straight_length + TAU * self.bend_radius
    }

    /// Position at wrapped arc length; positive offset is toward the loop interior.
    pub fn at(&self, distance: f32, offset: f32) -> Vector {
        let mut s = distance.rem_euclid(self.length());
        if s < self.straight_length {
            return Vector::new(s, offset);
        }
        s -= self.straight_length;
        if s < PI * self.bend_radius {
            let angle = s / self.bend_radius - FRAC_PI_2;
            return Vector::new(self.straight_length, self.bend_radius)
                + Vector::new(angle.cos(), angle.sin()) * (self.bend_radius - offset);
        }
        s -= PI * self.bend_radius;
        if s < self.straight_length {
            return Vector::new(self.straight_length - s, 2.0 * self.bend_radius - offset);
        }
        s -= self.straight_length;
        let angle = s / self.bend_radius + FRAC_PI_2;
        Vector::new(0.0, self.bend_radius)
            + Vector::new(angle.cos(), angle.sin()) * (self.bend_radius - offset)
    }

    pub fn project(&self, position: Vector) -> Projection {
        if position.x < 0.0 || position.x > self.straight_length {
            let right = position.x > self.straight_length;
            let center = Vector::new(
                if right { self.straight_length } else { 0.0 },
                self.bend_radius,
            );
            let radial = position - center;
            let angle = radial.y.atan2(radial.x);
            let distance = if right {
                self.straight_length + (angle + FRAC_PI_2) * self.bend_radius
            } else {
                2.0 * self.straight_length
                    + PI * self.bend_radius
                    + (angle.rem_euclid(TAU) - FRAC_PI_2) * self.bend_radius
            };
            Projection {
                distance: distance.rem_euclid(self.length()),
                offset: self.bend_radius - radial.length(),
                tangent: Vector::new(-radial.y, radial.x) / radial.length(),
            }
        } else if position.y <= self.bend_radius {
            Projection {
                distance: position.x,
                offset: position.y,
                tangent: Vector::X,
            }
        } else {
            Projection {
                distance: 2.0 * self.straight_length + PI * self.bend_radius - position.x,
                offset: 2.0 * self.bend_radius - position.y,
                tangent: -Vector::X,
            }
        }
    }

    /// Outer and inner contours, explicitly closed; shared by collision and drawing.
    pub fn wall_contours(&self) -> Vec<Vec<[f32; 2]>> {
        [-self.half_width, self.half_width]
            .into_iter()
            .map(|offset| {
                (0..=128)
                    .map(|i| self.at(self.length() * i as f32 / 128.0, offset).to_array())
                    .collect()
            })
            .collect()
    }
}
