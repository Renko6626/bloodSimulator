use crate::{
    Species, WhiteCellOptions,
    behavior::{ActiveBehavior, Seen, target_id},
    entity::TargetRef,
};
use rapier2d::prelude::Vector;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThreatResponse {
    #[default]
    Retaliate,
    Flee,
}

#[derive(Clone, Copy, Debug)]
pub struct BacteriumOptions {
    pub response: ThreatResponse,
    /// Simulated seconds of continued escape after losing sight of the threat.
    pub flee_duration: f32,
}

impl Default for BacteriumOptions {
    fn default() -> Self {
        Self {
            response: ThreatResponse::Retaliate,
            flee_duration: 0.8,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BacteriaOptions {
    pub motile: BacteriumOptions,
    pub encapsulated: BacteriumOptions,
}

impl BacteriaOptions {
    pub(crate) fn for_species(&self, species: Species) -> &BacteriumOptions {
        match species {
            Species::MotileBacterium => &self.motile,
            Species::EncapsulatedBacterium => &self.encapsulated,
            _ => unreachable!("only bacteria have a threat response"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Bacterium {
    pub active: ActiveBehavior,
    pub target_position: Option<Vector>,
    pub threat: Option<TargetRef>,
    escape_direction: Vector,
    escape_remaining: f32,
}

impl Bacterium {
    pub fn new(id: u64) -> Self {
        Self {
            active: ActiveBehavior::new(id),
            target_position: None,
            threat: None,
            escape_direction: Vector::ZERO,
            escape_remaining: 0.0,
        }
    }

    pub fn react_to_hit(
        &mut self,
        source: TargetRef,
        position: Vector,
        center: Vector,
        visible: bool,
        response: &BacteriumOptions,
    ) {
        match response.response {
            ThreatResponse::Retaliate => {
                self.threat = visible.then_some(source);
                self.active.set_target(self.threat);
                self.target_position = visible.then_some(position);
            }
            ThreatResponse::Flee => {
                self.threat = Some(source);
                self.escape_direction = self.away_from(center, position);
                self.escape_remaining = response.flee_duration;
                self.stop_attacking();
            }
        }
    }

    fn away_from(&self, center: Vector, threat: Vector) -> Vector {
        let direction = (center - threat).normalize_or_zero();
        if direction == Vector::ZERO {
            self.active.heading
        } else {
            direction
        }
    }

    fn stop_attacking(&mut self) {
        self.active.set_target(None);
        self.active.contact_time = 0.0;
        self.target_position = None;
    }

    pub fn think(
        &mut self,
        center: Vector,
        visible: &[Seen],
        dt: f32,
        options: &WhiteCellOptions,
        response: &BacteriumOptions,
    ) {
        self.active.attack_cooldown = (self.active.attack_cooldown - dt).max(0.0);
        let attacker = self
            .threat
            .and_then(|t| visible.iter().find(|f| f.target == t));
        if response.response == ThreatResponse::Flee && self.threat.is_some() {
            if let Some(threat) = attacker {
                self.escape_direction = self.away_from(center, threat.position);
                self.escape_remaining = response.flee_duration;
            } else {
                self.escape_remaining = (self.escape_remaining - dt).max(0.0);
            }
            if attacker.is_some() || self.escape_remaining > 0.0 {
                self.stop_attacking();
                self.active.steer(
                    self.escape_direction,
                    options.chase_speed,
                    dt,
                    options.turn_time,
                );
                return;
            }
        }
        if attacker.is_none() {
            self.threat = None;
        }
        let chosen = attacker
            .or_else(|| {
                visible.iter().find(|f| {
                    f.species == Some(Species::RedCell) && Some(f.target) == self.active.target
                })
            })
            .or_else(|| {
                visible
                    .iter()
                    .filter(|f| f.species == Some(Species::RedCell))
                    .min_by(|a, b| {
                        center
                            .distance_squared(a.position)
                            .total_cmp(&center.distance_squared(b.position))
                            .then_with(|| target_id(a.target).cmp(&target_id(b.target)))
                    })
            });
        self.active.set_target(chosen.map(|f| f.target));
        self.target_position = chosen.map(|f| f.position);
        let (direction, speed) = match chosen {
            Some(f) => (
                (f.position - center).normalize_or_zero(),
                options.chase_speed,
            ),
            None => (self.active.patrol(dt), options.patrol_speed),
        };
        self.active.steer(direction, speed, dt, options.turn_time);
    }
}
