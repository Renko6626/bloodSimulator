//! Independent, fixed-step simulation. Distances are local cell-scale units;
//! time is seconds, with +Y pointing down to match the experiment view.

mod cell;
pub mod circulation;
mod config;
pub mod map;
mod robot;
mod runtime;
pub mod vessel;
mod view;

pub use config::Config;
pub use runtime::{MoveInput, Simulation};
pub use view::{FlowArrow, Snapshot};
