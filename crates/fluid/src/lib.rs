//! Solver-independent Stokes data, CPU backends, and shared numerical checks.
pub mod block;
pub mod cpu;
pub mod dataset;
pub mod krylov;
pub mod lu;
pub mod matrix;
pub mod multigrid;
pub mod problem;

pub use matrix::CsrMatrix;
pub use problem::{Geometry, StokesProblem};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
pub type FluidResult<T> = Result<T, String>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SolveOptions {
    pub tolerance: f64,
    pub restart: usize,
    pub max_iterations: usize,
    pub v_cycles: usize,
}
impl Default for SolveOptions {
    fn default() -> Self {
        Self {
            tolerance: 1e-4,
            restart: 32,
            max_iterations: 200,
            v_cycles: 2,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SolverCounters {
    pub dispatches: u64,
    pub submissions: u64,
    pub readbacks: u64,
    pub readback_bytes: u64,
    /// Overlaps GPU execution and overall elapsed time; not pure transfer time.
    pub readback_wait_ms: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolveReport {
    pub backend: String,
    pub status: String,
    pub iterations: usize,
    pub stopping_relative: Option<f64>,
    pub multigrid_levels: Vec<usize>,
    pub counters: SolverCounters,
}
impl SolveReport {
    pub fn converged(&self) -> bool {
        self.status == "converged"
    }
}

pub trait PreparedSolver: Send {
    fn solve(&mut self, rhs: &[f64], output: &mut [f64]) -> FluidResult<SolveReport>;
}
pub trait SolverFactory: Send + Sync {
    fn name(&self) -> &'static str;
    fn prepare(&self, problem: Arc<StokesProblem>) -> FluidResult<Box<dyn PreparedSolver>>;
    fn device(&self) -> Option<DeviceInfo> {
        None
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct DeviceInfo {
    pub name: String,
    pub backend: String,
    pub kind: String,
    pub software: bool,
}
