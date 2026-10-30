use crate::circulation::Flow;

#[derive(Clone, Debug)]
pub struct FlowArrow {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
}

/// Dynamic display data only; stable vessel contours are retrieved separately.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub streaming: Option<crate::StreamingSnapshot>,
    pub robot: [f32; 2],
    pub cells: Vec<Vec<[f32; 2]>>,
    /// Biological/display metadata in exactly the same order as `cells`.
    pub cell_status: Vec<CellStatus>,
    pub attack: Option<AttackSnapshot>,
    pub immunity: ImmunitySnapshot,
    pub population: Option<CellPopulationSnapshot>,
    pub arrows: Vec<FlowArrow>,
    pub steps: u64,
    pub time: f32,
    pub paused: bool,
    pub solve_ms: f64,
    /// Soft/soft contact witnesses, not the number of distinct contacting pairs.
    pub soft_contacts: usize,
    pub flow: Flow,
    /// Per-edge flow rates in a world map; empty in the original loop lab.
    pub branch_flows: Vec<f32>,
    pub fluid: Option<crate::FluidStats>,
    pub lbm: Option<LbmSnapshot>,
    /// Diagnostic calculation error, if the cached field is invalid.
    pub lbm_status: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CellStatus {
    pub id: u64,
    pub white: bool,
    pub pursuing: bool,
    pub contact_progress: f32,
    pub call_progress: f32,
    pub health: f32,
    pub max_health: f32,
    pub dead: bool,
    pub opacity: f32,
}

#[derive(Clone, Debug)]
pub struct ImmunitySnapshot {
    pub alertness: f32,
    pub robot_health: f32,
    pub robot_max_health: f32,
    pub white_bodies: usize,
    pub arrival: Option<ArrivalSnapshot>,
}

#[derive(Clone, Debug)]
pub struct ArrivalSnapshot {
    pub wall: [f32; 2],
    pub position: [f32; 2],
    pub progress: f32,
}

#[derive(Clone, Debug)]
pub struct AttackSnapshot {
    pub center: [f32; 2],
    pub radius: f32,
    pub opacity: f32,
}

/// Cached local-fluid diagnostics, expressed in world coordinates.
#[derive(Clone, Debug)]
pub struct LbmSnapshot {
    pub profile: String,
    pub origin_blocks: [i32; 2],
    pub bounds: [[f32; 2]; 2],
    pub opening_targets: Vec<f32>,
    pub opening_fluxes: Vec<f32>,
    pub opening_residuals: Vec<f32>,
    pub density_min: f32,
    pub density_max: f32,
    pub mass: f32,
    pub region_switches: usize,
    pub overlap_error: f32,
    pub center: [f32; 3],
    pub warmup: Vec<[f32; 3]>,
    pub lattice_steps_per_substep: usize,
    pub lattice_dt: f32,
    pub elapsed_fluid_time: f64,
    pub coupling_ms: f64,
    pub lbm_ms: f64,
    pub rapier_ms: f64,
    pub adapter_name: String,
    pub software_adapter: bool,
}

/// Quantities in local-cell mode. Abstract stock includes far traffic and any
/// near-field fractions/blocked stock; it is counted exactly once.
#[derive(Clone, Debug)]
pub struct CellPopulationSnapshot {
    pub total_count: usize,
    pub detailed_count: usize,
    pub abstract_count: f64,
    pub spawned_this_frame: usize,
    pub retired_this_frame: usize,
    pub exchange_ms: f64,
}
