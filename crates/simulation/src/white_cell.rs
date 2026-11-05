use crate::behavior::ActiveBehavior;
use rapier2d::prelude::Vector;

/// Early-game tuning, in the same units and simulation clock as Config.
#[derive(Clone, Debug)]
pub struct WhiteCellOptions {
    pub count: usize,
    pub radius: f32,
    pub health: f32,
    pub patrol_speed: f32,
    pub investigate_speed: f32,
    pub chase_speed: f32,
    pub turn_time: f32,
    pub contact_warmup: f32,
    pub attack_interval: f32,
    pub damage: f32,
    pub call_time: f32,
    pub call_interval: f32,
    pub max_bodies: usize,
}

impl Default for WhiteCellOptions {
    fn default() -> Self {
        Self {
            count: 4,
            radius: 1.0,
            health: 150.0,
            patrol_speed: 0.6,
            investigate_speed: 2.0,
            chase_speed: 3.0,
            turn_time: 0.3,
            contact_warmup: 0.3,
            attack_interval: 0.6,
            damage: 8.0,
            call_time: 1.0,
            call_interval: 8.0,
            max_bodies: 8,
        }
    }
}

pub(crate) const WANTED_THRESHOLD: f32 = 50.0;
pub(crate) const ARRIVAL_TIME: f32 = 0.8;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Goal {
    Investigate(Vector),
    Pursue(Vector),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WhiteCell {
    pub goal: Option<Goal>,
    pub active: ActiveBehavior,
    pub attacker: Option<crate::entity::TargetRef>,
    pub memory: f32,
    pub call_time: f32,
    pub arrival_time: f32,
}

impl WhiteCell {
    pub fn new(id: u64) -> Self {
        Self {
            goal: None,
            active: ActiveBehavior::new(id),
            attacker: None,
            memory: 0.0,
            call_time: 0.0,
            arrival_time: 0.0,
        }
    }

    pub fn pursue(&mut self, position: Vector, alertness: f32) {
        self.set_target(Some(crate::entity::TargetRef::Robot));
        self.goal = Some(Goal::Pursue(position));
        self.memory = 2.0 + 3.0 * alertness / 100.0;
    }

    pub fn set_target(&mut self, target: Option<crate::entity::TargetRef>) {
        // Direct-attack evidence belongs to the target being pursued. Once
        // another target is chosen (or pursuit ends), its memory cannot keep
        // an abandoned source eligible for later retaliation.
        if self.attacker != target {
            self.attacker = None;
        }
        self.active.set_target(target);
    }

    pub fn think(
        &mut self,
        center: Vector,
        visible: Option<(crate::entity::TargetRef, Vector)>,
        alertness: f32,
        dt: f32,
        options: &WhiteCellOptions,
    ) {
        self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        self.arrival_time = (self.arrival_time - dt).max(0.0);
        if let Some((target, point)) = visible {
            self.set_target(Some(target));
            self.goal = Some(Goal::Pursue(point));
            self.memory = 2.0 + 3.0 * alertness / 100.0;
        } else {
            self.contact_time = 0.0;
            if let Some(Goal::Investigate(point) | Goal::Pursue(point)) = self.goal {
                self.memory -= dt;
                if self.memory <= 0.0 || center.distance(point) < 0.6 {
                    self.goal = None;
                    self.attacker = None;
                    self.set_target(None);
                }
            }
        }
        let (desired, speed) = match self.goal {
            Some(Goal::Pursue(point)) => {
                ((point - center).normalize_or_zero(), options.chase_speed)
            }
            Some(Goal::Investigate(point)) => (
                (point - center).normalize_or_zero(),
                options.investigate_speed,
            ),
            None => (self.active.patrol(dt), options.patrol_speed),
        };
        self.active.steer(desired, speed, dt, options.turn_time);
    }

    pub fn is_pursuing(&self) -> bool {
        matches!(self.goal, Some(Goal::Pursue(_)))
    }
}

impl std::ops::Deref for WhiteCell {
    type Target = ActiveBehavior;
    fn deref(&self) -> &Self::Target {
        &self.active
    }
}
impl std::ops::DerefMut for WhiteCell {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.active
    }
}

#[derive(Default)]
pub(crate) struct Immunity {
    pub alertness: f32,
    pub quiet_time: f32,
    pub call_cooldown: f32,
    pub arrival: Option<Arrival>,
}

pub(crate) struct Arrival {
    pub wall: Vector,
    pub position: Vector,
    pub remaining: f32,
}

impl Immunity {
    pub fn advance(&mut self, dt: f32) {
        let previous = (self.quiet_time - 6.0).max(0.0);
        self.quiet_time += dt;
        let decay_time = (self.quiet_time - 6.0).max(0.0) - previous;
        self.alertness = (self.alertness - 2.0 * decay_time).max(0.0);
        self.call_cooldown = (self.call_cooldown - dt).max(0.0);
        if let Some(arrival) = &mut self.arrival {
            arrival.remaining -= dt;
            if arrival.remaining <= 0.0 {
                self.arrival = None;
            }
        }
    }

    pub fn sense_radius(&self) -> f32 {
        6.0 + 4.0 * self.alertness / 100.0
    }
}
