use super::*;

fn zero_config(config: &mut Config) {
    config.pressure = 0.;
    config.pulse_hz = 0.;
    config.cell_count = 0;
    config.white_cells.count = 0;
    config.cell_mode = CellSimulationMode::Full;
}
impl Simulation {
    pub fn new_heart_preview(mut config: Config) -> Self {
        zero_config(&mut config);
        let mut sim = Self::new_streamed_world(config, MapDefinition::heart_preview());
        sim.heart_preview = true;
        sim.white_cells_enabled = false;
        sim
    }
    pub fn is_heart_preview(&self) -> bool {
        self.heart_preview
    }
    pub(super) fn enforce_heart_preview(&mut self) {
        zero_config(&mut self.config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_zero_state_survives_input_pause_and_reset() {
        let mut sim = Simulation::new_heart_preview(Config::default());
        sim.config.pressure = 20.;
        sim.config.pulse_hz = 2.;
        sim.config.cell_count = 12;
        sim.config.white_cells.count = 5;
        sim.set_paused(true);
        sim.single_step(MoveInput::default());
        assert_eq!(sim.config.pressure, 0.);
        assert_eq!(sim.config.pulse_hz, 0.);
        assert_eq!(sim.config.cell_count, 0);
        assert_eq!(sim.config.white_cells.count, 0);
        assert!(sim.cells.is_empty());
        assert_eq!(sim.snapshot().immunity.white_bodies, 0);
        assert_eq!(sim.environment.summary().pressure_drop, 0.);
        sim.reset();
        assert!(sim.is_heart_preview());
        assert!(sim.paused);
        assert_eq!(sim.world_map().unwrap().chambers.len(), 4);
        assert!(sim.cells.is_empty());
        assert_eq!(sim.snapshot().immunity.white_bodies, 0);
    }
}
