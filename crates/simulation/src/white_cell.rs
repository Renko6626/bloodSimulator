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
    pub heading: Vector,
    pub movement: Vector,
    pub memory: f32,
    pub patrol_time: f32,
    pub contact_time: f32,
    pub attack_cooldown: f32,
    pub call_time: f32,
    pub arrival_time: f32,
}

impl WhiteCell {
    pub fn new(id: u64) -> Self {
        let angle = id as f32 * 2.399_963_1;
        Self {
            goal: None,
            heading: Vector::new(angle.cos(), angle.sin()),
            movement: Vector::ZERO,
            memory: 0.0,
            patrol_time: 2.0,
            contact_time: 0.0,
            attack_cooldown: 0.0,
            call_time: 0.0,
            arrival_time: 0.0,
        }
    }

    pub fn pursue(&mut self, position: Vector, alertness: f32) {
        self.goal = Some(Goal::Pursue(position));
        self.memory = 2.0 + 3.0 * alertness / 100.0;
    }

    /// Only receives the robot position when the coordinator can actually sense it.
    pub fn think(
        &mut self,
        center: Vector,
        visible_robot: Option<Vector>,
        alertness: f32,
        dt: f32,
        options: &WhiteCellOptions,
    ) {
        self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        self.arrival_time = (self.arrival_time - dt).max(0.0);
        let can_track = self.is_pursuing() || alertness >= WANTED_THRESHOLD;
        if let Some(robot) = visible_robot.filter(|_| can_track) {
            self.pursue(robot, alertness);
        } else if let Some(goal) = self.goal {
            self.memory -= dt;
            let (Goal::Investigate(point) | Goal::Pursue(point)) = goal;
            if self.memory <= 0.0 || center.distance(point) < 0.6 {
                self.goal = None;
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
            None => {
                self.patrol_time -= dt;
                if self.patrol_time <= 0.0 {
                    self.patrol_time = 2.0;
                    self.heading = rotate(self.heading, 0.55);
                }
                (self.heading, options.patrol_speed)
            }
        };
        if desired != Vector::ZERO {
            let angle = self
                .heading
                .perp_dot(desired)
                .atan2(self.heading.dot(desired));
            let max_turn = std::f32::consts::PI * dt / options.turn_time;
            self.heading = rotate(self.heading, angle.clamp(-max_turn, max_turn));
        }
        self.movement = self.heading * speed;
    }

    pub fn is_pursuing(&self) -> bool {
        matches!(self.goal, Some(Goal::Pursue(_)))
    }
}

fn rotate(v: Vector, angle: f32) -> Vector {
    let (s, c) = angle.sin_cos();
    Vector::new(c * v.x - s * v.y, s * v.x + c * v.y)
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
