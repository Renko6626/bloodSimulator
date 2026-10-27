//! Independent, fixed-step simulation. Distances are local cell-scale units;
//! time is seconds, with +Y pointing down to match the experiment view.

mod cell;
pub mod circulation;
mod config;
mod fluid;
pub mod map;
mod robot;
mod runtime;
mod threading;
pub mod vessel;
mod view;

pub use config::Config;
#[cfg(feature = "lbm-gpu")]
pub use fluid::lbm_geometry;
pub use fluid::{FluidStats, LocalFluidOptions};
pub use runtime::{MoveInput, Simulation};
pub use view::{FlowArrow, Snapshot};
