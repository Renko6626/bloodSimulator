use crate::white_cell::WhiteCell;

/// Both active families share local steering and contact timing, while species
/// determines legal targets and damage rules in the runtime coordinator.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Behavior {
    Passive,
    Immune(WhiteCell),
    Bacterium(WhiteCell),
}
impl Behavior {
    pub fn as_ref(&self) -> Option<&WhiteCell> {
        match self {
            Self::Passive => None,
            Self::Immune(b) | Self::Bacterium(b) => Some(b),
        }
    }
    pub fn as_mut(&mut self) -> Option<&mut WhiteCell> {
        match self {
            Self::Passive => None,
            Self::Immune(b) | Self::Bacterium(b) => Some(b),
        }
    }
}
