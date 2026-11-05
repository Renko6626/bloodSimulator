use crate::{Species, entity::TargetRef, pathogen::Bacterium, white_cell::WhiteCell};
use rapier2d::prelude::Vector;

/// Facts prepared by the runtime; decision code never queries the world itself.
#[derive(Clone, Copy)]
pub(crate) struct Seen {
    pub target: TargetRef,
    pub species: Option<Species>,
    pub position: Vector,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ActiveBehavior {
    pub target: Option<TargetRef>,
    pub heading: Vector,
    pub movement: Vector,
    pub patrol_time: f32,
    pub contact_time: f32,
    pub attack_cooldown: f32,
}

impl ActiveBehavior {
    pub fn new(id: u64) -> Self {
        let angle = id as f32 * 2.399_963_1;
        Self {
            target: None,
            heading: Vector::new(angle.cos(), angle.sin()),
            movement: Vector::ZERO,
            patrol_time: 2.0,
            contact_time: 0.0,
            attack_cooldown: 0.0,
        }
    }

    pub fn set_target(&mut self, target: Option<TargetRef>) {
        if self.target != target {
            self.contact_time = 0.0;
        }
        self.target = target;
    }

    pub fn patrol(&mut self, dt: f32) -> Vector {
        self.patrol_time -= dt;
        if self.patrol_time <= 0.0 {
            self.patrol_time = 2.0;
            self.heading = rotate(self.heading, 0.55);
        }
        self.heading
    }

    pub fn steer(&mut self, desired: Vector, speed: f32, dt: f32, turn_time: f32) {
        if desired != Vector::ZERO {
            let angle = self
                .heading
                .perp_dot(desired)
                .atan2(self.heading.dot(desired));
            let max_turn = std::f32::consts::PI * dt / turn_time;
            self.heading = rotate(self.heading, angle.clamp(-max_turn, max_turn));
        }
        self.movement = self.heading * speed;
    }
}

pub(crate) fn rotate(v: Vector, angle: f32) -> Vector {
    let (s, c) = angle.sin_cos();
    Vector::new(c * v.x - s * v.y, s * v.x + c * v.y)
}

pub(crate) fn target_id(target: TargetRef) -> u64 {
    match target {
        TargetRef::Robot => u64::MAX,
        TargetRef::Cell(id) => id,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Behavior {
    Passive,
    Immune(WhiteCell),
    Bacterium(Bacterium),
}

impl Behavior {
    pub fn active(&self) -> Option<&ActiveBehavior> {
        match self {
            Self::Passive => None,
            Self::Immune(b) => Some(&b.active),
            Self::Bacterium(b) => Some(&b.active),
        }
    }

    pub fn active_mut(&mut self) -> Option<&mut ActiveBehavior> {
        match self {
            Self::Passive => None,
            Self::Immune(b) => Some(&mut b.active),
            Self::Bacterium(b) => Some(&mut b.active),
        }
    }

    pub fn immune(&self) -> Option<&WhiteCell> {
        match self {
            Self::Immune(b) => Some(b),
            _ => None,
        }
    }

    pub fn immune_mut(&mut self) -> Option<&mut WhiteCell> {
        match self {
            Self::Immune(b) => Some(b),
            _ => None,
        }
    }

    pub fn target_position(&self) -> Option<Vector> {
        match self {
            Self::Passive => None,
            Self::Immune(b) => b.goal.map(|g| match g {
                crate::white_cell::Goal::Investigate(p) | crate::white_cell::Goal::Pursue(p) => p,
            }),
            Self::Bacterium(b) => b.target_position,
        }
    }
}
