use crate::circulation::Flow;

#[derive(Clone, Debug)]
pub struct FlowArrow {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
}

/// Dynamic display data only; stable vessel contours are retrieved separately.
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub robot: [f32; 2],
    pub cells: Vec<Vec<[f32; 2]>>,
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
}
