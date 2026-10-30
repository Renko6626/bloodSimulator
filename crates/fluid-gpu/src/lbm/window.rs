//! Compatibility wrapper for the existing block-aligned realtime window.
use super::{
    Diagnostics, InitialField, LbmDomain, LbmResult, LbmWorkStats, LocalSample, MeanDensityControl,
    StepInput, WindowConfig, WindowGeometry, WindowOrigin, WindowSpec,
};

pub struct LbmWindow {
    spec: WindowSpec,
    domain: LbmDomain,
}

impl LbmWindow {
    pub fn new(
        spec: WindowSpec,
        config: WindowConfig,
        initial: InitialField,
        geometry: WindowGeometry,
    ) -> LbmResult<Self> {
        Ok(Self {
            spec,
            domain: LbmDomain::new(spec.domain(), config, initial, geometry)?,
        })
    }
    pub fn spec(&self) -> WindowSpec {
        self.spec
    }
    pub fn shift_to(
        &mut self,
        origin: WindowOrigin,
        initial: InitialField,
        geometry: WindowGeometry,
    ) -> LbmResult<()> {
        let new = WindowSpec {
            origin,
            ..self.spec
        };
        self.domain.shift_to(
            new.domain(),
            new.origin_cell_offset_from(self.spec),
            initial,
            geometry,
        )?;
        self.spec = new;
        Ok(())
    }
    pub fn reset_work_stats(&self) {
        self.domain.reset_work_stats()
    }
    pub fn work_stats(&self) -> LbmWorkStats {
        self.domain.work_stats()
    }
    pub fn control_mean_density(&self) -> LbmResult<MeanDensityControl> {
        self.domain.control_mean_density()
    }
    pub fn set_opening_profiles(&mut self, flows: &[f32], profiles: &[Vec<f32>]) -> LbmResult<()> {
        self.domain.set_opening_profiles(flows, profiles)
    }
    pub fn geometry(&self) -> &WindowGeometry {
        self.domain.geometry()
    }
    pub fn apply_kicks(&mut self, kicks: &[[f32; 2]]) -> LbmResult<()> {
        self.domain.apply_kicks(kicks)
    }
    pub fn advance(&mut self, input: StepInput) -> LbmResult<()> {
        self.domain.advance(input)
    }
    pub fn advance_batch(&mut self, input: StepInput, count: usize) -> LbmResult<()> {
        self.domain.advance_batch(input, count)
    }
    pub fn field(&self) -> LbmResult<Vec<[f32; 4]>> {
        self.domain.field()
    }
    pub fn sample(&self, position: [f32; 2]) -> LbmResult<LocalSample> {
        self.domain.sample(position)
    }
    pub fn snapshot_diagnostics(&self) -> LbmResult<Diagnostics> {
        self.domain.snapshot_diagnostics()
    }
    pub fn diagnostics_from_field(&self, field: &[[f32; 4]]) -> LbmResult<Diagnostics> {
        self.domain.diagnostics_from_field(field)
    }
    pub fn opening_targets(&self) -> Vec<f32> {
        self.domain.opening_targets()
    }
    #[cfg(test)]
    pub(super) fn populations_for_test(&self) -> Vec<f32> {
        self.domain.populations_for_test()
    }
}
