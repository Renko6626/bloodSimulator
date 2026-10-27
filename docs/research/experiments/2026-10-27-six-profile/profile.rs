//! Research-only counters for one sequential Simulation, excluding snapshots.
//! Geometry/flow spans nest; sparse-factor work may overlap across two threads.
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

pub const NAMES: [&str; 43] = [
    "analytic_total",
    "nearest_edge",
    "signed_distance",
    "project_edge",
    "edge_length",
    "junction_search",
    "junction_mix",
    "geometry_unused7",
    "geometry_unused8",
    "geometry_unused9",
    "fluid_advance",
    "marker_prepare",
    "marker_spread",
    "grid_solve",
    "combine_field",
    "full_diagnostics",
    "impulse_and_finish",
    "pressure_gradient",
    "divergence",
    "schur",
    "true_measure",
    "velocity_inverse",
    "factor_total_work",
    "factor_reorder_work",
    "factor_forward_work",
    "factor_backward_work",
    "factor_restore_work",
    "u_solve",
    "v_solve",
    "max_component_per_inverse",
    "rapier_update",
    "rapier_collision",
    "rapier_islands",
    "rapier_constraint_maintenance",
    "rapier_solver",
    "rapier_user_changes",
    "rapier_ccd",
    "rapier_broad",
    "rapier_narrow",
    "rapier_final_broad",
    "rapier_assembly",
    "rapier_resolution",
    "rapier_writeback",
];
pub const N: usize = NAMES.len();
static MODE: AtomicUsize = AtomicUsize::new(0);
static NS: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];
static CALLS: [AtomicU64; N] = [const { AtomicU64::new(0) }; N];

#[derive(Clone, Debug)]
pub struct DeepStats {
    pub ns: [u64; N],
    pub calls: [u64; N],
}
impl Default for DeepStats {
    fn default() -> Self {
        Self {
            ns: [0; N],
            calls: [0; N],
        }
    }
}

pub fn begin() {
    for i in 0..N {
        NS[i].store(0, Relaxed);
        CALLS[i].store(0, Relaxed);
    }
    let mode = match std::env::var("DEEP_MODE").as_deref() {
        Ok("geometry") => 1,
        Ok("fluid") => 2,
        Ok("rapier") => 3,
        _ => 0,
    };
    MODE.store(mode, Relaxed);
}
pub fn mode() -> usize {
    MODE.load(Relaxed)
}
pub fn finish() -> DeepStats {
    // All parallel factor tasks have joined before this call.
    MODE.store(0, Relaxed);
    DeepStats {
        ns: std::array::from_fn(|i| NS[i].load(Relaxed)),
        calls: std::array::from_fn(|i| CALLS[i].load(Relaxed)),
    }
}
pub fn add(id: usize, ns: u64) {
    NS[id].fetch_add(ns, Relaxed);
    CALLS[id].fetch_add(1, Relaxed);
}
pub struct Span {
    id: usize,
    start: Option<Instant>,
}
pub fn span(id: usize) -> Span {
    let expected = if id < 10 {
        1
    } else if id < 30 {
        2
    } else {
        3
    };
    Span {
        id,
        start: (mode() == expected).then(Instant::now),
    }
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            add(self.id, start.elapsed().as_nanos() as u64);
        }
    }
}
