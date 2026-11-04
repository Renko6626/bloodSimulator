use crate::{Config, WhiteCellOptions};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EntityCore {
    pub id: u64,
    pub health: f32,
    pub max_health: f32,
    pub dead_age: Option<f32>,
}
impl EntityCore {
    pub fn new(id: u64, health: f32) -> Self {
        Self {
            id,
            health,
            max_health: health,
            dead_age: None,
        }
    }
    pub fn damage(&mut self, amount: f32) {
        if self.dead_age.is_some() {
            return;
        }
        self.health = (self.health - amount.max(0.0)).max(0.0);
        if self.health == 0.0 {
            self.dead_age = Some(0.0);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Species {
    RedCell,
    Neutrophil,
    Macrophage,
    MotileBacterium,
    EncapsulatedBacterium,
}
impl Species {
    pub fn is_immune(self) -> bool {
        matches!(self, Self::Neutrophil | Self::Macrophage)
    }
    pub fn is_pathogen(self) -> bool {
        matches!(self, Self::MotileBacterium | Self::EncapsulatedBacterium)
    }
    pub fn radius(self, config: &Config) -> f32 {
        match self {
            Self::RedCell => config.cell_radius,
            Self::Neutrophil => config.white_cells.radius,
            Self::Macrophage => 1.5,
            Self::MotileBacterium => 0.35,
            Self::EncapsulatedBacterium => 0.5,
        }
    }
    pub(crate) fn options(self, config: &Config) -> WhiteCellOptions {
        let mut o = config.white_cells.clone();
        match self {
            Self::RedCell | Self::Neutrophil => return o,
            Self::Macrophage => {
                o.health = 450.;
                o.patrol_speed = 0.25;
                o.chase_speed = 1.2;
                o.turn_time = 0.8;
                o.contact_warmup = 0.9;
                o.damage = 30.;
                o.attack_interval = 1.2;
            }
            Self::MotileBacterium => {
                o.health = 50.;
                o.patrol_speed = 0.8;
                o.chase_speed = 1.8;
                o.turn_time = 0.3;
                o.contact_warmup = 0.4;
                o.damage = 5.;
                o.attack_interval = 0.8;
            }
            Self::EncapsulatedBacterium => {
                o.health = 100.;
                o.patrol_speed = 0.1;
                o.chase_speed = 0.35;
                o.turn_time = 0.6;
                o.contact_warmup = 0.7;
                o.damage = 8.;
                o.attack_interval = 1.;
            }
        }
        o.radius = self.radius(config);
        o.investigate_speed = o.chase_speed;
        o
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TargetRef {
    Robot,
    Cell(u64),
}
