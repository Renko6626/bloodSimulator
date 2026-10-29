/// Experimental values, not biological measurements.
#[derive(Clone, Debug)]
pub struct Config {
    pub dt: f32,
    pub substeps: usize,
    pub cell_count: usize,
    pub cell_mode: crate::CellSimulationMode,
    pub cell_radius: f32,
    pub cell_vertices: usize,
    pub cell_frequency: f32,
    pub cell_area_frequency: f32,
    pub cell_damping: f32,
    pub cell_drag: f32,
    pub robot_speed: f32,
    pub robot_flow: f32,
    pub pressure: f32,
    pub viscosity: f32,
    pub pulse_hz: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            dt: 1.0 / 60.0,
            substeps: 2,
            cell_count: 72,
            cell_mode: crate::CellSimulationMode::Full,
            cell_radius: 0.7,
            cell_vertices: 12,
            cell_frequency: 12.0,
            cell_area_frequency: 30.0,
            cell_damping: 0.5,
            cell_drag: 20.0,
            robot_speed: 4.0,
            robot_flow: 0.08,
            pressure: 12.0,
            viscosity: 0.05,
            pulse_hz: 1.2,
        }
    }
}
