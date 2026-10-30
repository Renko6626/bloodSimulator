//! Independent, fixed-step simulation. Distances are local cell-scale units;
//! time is seconds, with +Y pointing down to match the experiment view.

mod background;
mod cell;
mod cell_population;
pub mod circulation;
mod combat;
mod config;
mod fluid;
#[cfg(feature = "lbm-gpu")]
pub mod lbm_validation;
pub mod map;
mod robot;
mod runtime;
mod threading;
pub mod vessel;
mod view;
mod white_cell;
mod world_streaming;

#[cfg(feature = "lbm-gpu")]
pub use blood_fluid_gpu::lbm::GridProfile;
pub use cell_population::{CellPopulationOptions, CellSimulationMode};
pub use config::Config;
#[cfg(feature = "lbm-gpu")]
pub use fluid::lbm_coupling::{LbmOptions, LbmStats};
pub use fluid::{FluidStats, LocalFluidOptions};
#[cfg(feature = "lbm-gpu")]
pub use fluid::{lbm_geometry, streaming_window};
pub use runtime::{MoveInput, PolicyFluidStatus, Simulation};
pub use view::{
    ArrivalSnapshot, AttackSnapshot, CellPopulationSnapshot, CellStatus, FlowArrow,
    ImmunitySnapshot, LbmSnapshot, Snapshot,
};
pub use white_cell::WhiteCellOptions;
pub use world_streaming::{StreamingSnapshot, WorldChunkDelta};
