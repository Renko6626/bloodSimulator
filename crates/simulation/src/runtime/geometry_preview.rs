use super::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum GeometryPreview {
    Heart,
    Organ,
}

fn zero_config(config: &mut Config) {
    config.pressure = 0.;
    config.pulse_hz = 0.;
    config.cell_count = 0;
    config.white_cells.count = 0;
    config.cell_mode = CellSimulationMode::Full;
}
impl Simulation {
    pub fn new_heart_preview(config: Config) -> Self {
        Self::new_geometry_preview(config, GeometryPreview::Heart)
    }
    pub fn new_organ_preview(config: Config) -> Self {
        Self::new_geometry_preview(config, GeometryPreview::Organ)
    }
    pub(super) fn new_geometry_preview(mut config: Config, preview: GeometryPreview) -> Self {
        zero_config(&mut config);
        let map = match preview {
            GeometryPreview::Heart => MapDefinition::heart_preview(),
            GeometryPreview::Organ => MapDefinition::organ_world(),
        };
        let mut sim = Self::new_streamed_world(config, map);
        sim.geometry_preview = Some(preview);
        sim.white_cells_enabled = false;
        sim
    }
    pub fn is_heart_preview(&self) -> bool {
        self.geometry_preview == Some(GeometryPreview::Heart)
    }
    pub fn is_geometry_preview(&self) -> bool {
        self.geometry_preview.is_some()
    }
    pub(super) fn enforce_geometry_preview(&mut self) {
        zero_config(&mut self.config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_zero_state(sim: &Simulation) {
        assert_eq!(sim.config.pressure, 0.);
        assert_eq!(sim.config.pulse_hz, 0.);
        assert_eq!(sim.config.cell_count, 0);
        assert_eq!(sim.config.white_cells.count, 0);
        assert!(sim.cells.is_empty());
        let snapshot = sim.snapshot();
        assert_eq!(snapshot.immunity.immune_bodies, 0);
        assert_eq!(snapshot.flow.pressure_drop, 0.);
        assert_eq!(snapshot.flow.rate, 0.);
        assert_eq!(snapshot.flow.mean_speed, 0.);
    }

    #[test]
    fn both_previews_keep_zero_state_and_map_identity_on_step_and_reset() {
        for heart in [true, false] {
            let mut config = Config::default();
            config.cell_mode = CellSimulationMode::Local(Default::default());
            let mut sim = if heart {
                Simulation::new_heart_preview(config)
            } else {
                Simulation::new_organ_preview(config)
            };
            let expected = if heart {
                MapDefinition::heart_preview()
            } else {
                MapDefinition::organ_world()
            };
            assert_zero_state(&sim);
            sim.config.pressure = 20.;
            sim.config.pulse_hz = 2.;
            sim.config.cell_count = 12;
            sim.config.white_cells.count = 5;
            sim.tick(MoveInput { x: 1., y: 0. });
            assert_zero_state(&sim);
            sim.set_paused(true);
            sim.config.pressure = 30.;
            sim.single_step(MoveInput::default());
            assert_zero_state(&sim);
            sim.config.cell_count = 12;
            sim.config.white_cells.count = 5;
            sim.reset();
            assert!(sim.is_geometry_preview());
            assert_eq!(sim.is_heart_preview(), heart);
            assert!(sim.paused);
            let map = sim.world_map().unwrap();
            assert_eq!(map.spawn, expected.spawn);
            assert_eq!(map.edges.len(), expected.edges.len());
            assert_eq!(map.chambers.len(), 4);
            assert_eq!(sim.snapshot().robot, expected.spawn);
            assert_zero_state(&sim);
        }
    }
}
