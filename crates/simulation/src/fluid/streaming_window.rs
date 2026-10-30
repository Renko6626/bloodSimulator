//! Block-aligned local LBM window. The caller owns fixed-step scheduling.
use super::lbm_geometry::{MapWindowGeometry, opening_profiles, rasterize};
use crate::{circulation::graph::GraphFlow, map::MapDefinition};
use blood_fluid_gpu::lbm::{
    GridProfile, LbmWindow, LocalSample, LocalSampleStatus, StepInput, WindowConfig, WindowOrigin,
    WindowSpec,
};

const WARMUP_STEPS: usize = 8;

#[derive(Clone, Copy, Debug, Default)]
pub struct ContinuityDelta {
    pub density: f32,
    pub velocity: f32,
    pub momentum: f32,
}
#[derive(Clone, Debug, Default)]
pub struct MigrationDiagnostics {
    pub region_switches: usize,
    pub overlap_error: f32,
    pub center: ContinuityDelta,
    pub warmup: Vec<ContinuityDelta>,
}

pub struct StreamingWindow {
    window: LbmWindow,
    raster: MapWindowGeometry,
    field: Vec<[f32; 4]>,
    dt: f32,
    shifted: bool,
    frame_open: bool,
    field_pending: bool,
    density_control: Option<blood_fluid_gpu::lbm::MeanDensityControl>,
    baseline: Option<Vec<(usize, [f32; 4])>>,
    pub diagnostics: MigrationDiagnostics,
}

fn origin_for(position: [f32; 2]) -> Result<WindowOrigin, String> {
    let block = crate::map::CHUNK_SIZE;
    if !position.iter().all(|v| {
        v.is_finite() && *v >= (i32::MIN as f32 + 2.) * block && *v < (i32::MAX as f32 - 2.) * block
    }) {
        return Err("invalid player position".into());
    }
    Ok(WindowOrigin {
        block_x: (position[0] / block).floor() as i32 - 1,
        block_y: (position[1] / block).floor() as i32 - 1,
    })
}

impl StreamingWindow {
    pub fn new(
        map: &MapDefinition,
        flow: &GraphFlow,
        position: [f32; 2],
        profile: GridProfile,
        config: WindowConfig,
    ) -> Result<Self, String> {
        let spec = WindowSpec {
            origin: origin_for(position)?,
            profile,
        };
        let raster = rasterize(map, flow, spec)?;
        let mut window = LbmWindow::new(
            spec,
            config,
            raster.initial.clone(),
            raster.geometry.clone(),
        )?;
        let (targets, profiles) = opening_profiles(map, flow, &raster, spec)?;
        window.set_opening_profiles(&targets, &profiles)?;
        let field = window.field()?;
        Ok(Self {
            window,
            raster,
            field,
            dt: config.dt,
            shifted: false,
            frame_open: false,
            field_pending: false,
            density_control: None,
            baseline: None,
            diagnostics: MigrationDiagnostics::default(),
        })
    }
    pub fn active_origin(&self) -> WindowOrigin {
        self.window.spec().origin
    }
    pub fn bounds(&self) -> [[f32; 2]; 2] {
        self.window.spec().world_bounds()
    }
    /// Call exactly once at the start of each fixed physics step.
    pub fn begin_frame(&mut self) {
        self.frame_open = true;
        self.density_control = None;
        self.shifted = false;
        self.window.reset_work_stats();
    }
    /// Update current targets even when no migration is required.
    pub fn maybe_shift(
        &mut self,
        map: &MapDefinition,
        flow: &GraphFlow,
        position: [f32; 2],
    ) -> Result<bool, String> {
        if self.field_pending {
            return Err("finish deferred LBM frame before prepare".into());
        }
        let origin = origin_for(position)?;
        let old = self.window.spec();
        if !self.shifted && origin != old.origin {
            let spec = WindowSpec { origin, ..old };
            let raster = rasterize(map, flow, spec)?;
            let (targets, profiles) = opening_profiles(map, flow, &raster, spec)?;
            self.window
                .shift_to(origin, raster.initial.clone(), raster.geometry.clone())?;
            self.window.set_opening_profiles(&targets, &profiles)?;
            let new_field = self.window.field()?;
            let n = spec.profile.cells_per_side() as usize;
            let [dx, dy] = spec.origin_cell_offset_from(old);
            let mut baseline = Vec::new();
            let mut overlap_error = 0_f32;
            for y in 0..n {
                for x in 0..n {
                    let ox = x as i64 + dx;
                    let oy = y as i64 + dy;
                    if ox < 0 || oy < 0 || ox >= n as i64 || oy >= n as i64 {
                        continue;
                    }
                    let i = y * n + x;
                    let j = oy as usize * n + ox as usize;
                    if raster.geometry.solid[i] || self.raster.geometry.solid[j] {
                        continue;
                    }
                    for k in 0..3 {
                        overlap_error =
                            overlap_error.max((new_field[i][k] - self.field[j][k]).abs());
                    }
                    if x >= n / 3 && x < 2 * n / 3 && y >= n / 3 && y < 2 * n / 3 {
                        baseline.push((i, self.field[j]));
                    }
                }
            }
            self.diagnostics.region_switches += 1;
            self.diagnostics.overlap_error = overlap_error;
            self.diagnostics.center = delta(&baseline, &new_field);
            self.diagnostics.warmup.clear();
            self.baseline = Some(baseline);
            self.raster = raster;
            self.field = new_field;
            self.shifted = true;
            Ok(true)
        } else {
            let (targets, profiles) = opening_profiles(map, flow, &self.raster, old)?;
            self.window.set_opening_profiles(&targets, &profiles)?;
            Ok(false)
        }
    }
    /// Advance one lattice dt and refresh the one shared CPU sampling snapshot.
    pub fn advance(&mut self, input: StepInput) -> Result<(), String> {
        self.advance_batch(input, 1)
    }
    /// Advance a bounded number of lattice steps with one shared readback.
    pub fn advance_batch(&mut self, input: StepInput, count: usize) -> Result<(), String> {
        self.advance_batch_inner(input, count, false)
    }
    /// Last mechanical batch only: finish_frame must refresh the cache before reuse.
    pub(crate) fn advance_batch_deferred(
        &mut self,
        input: StepInput,
        count: usize,
    ) -> Result<(), String> {
        if !self.frame_open {
            return Err("LBM frame has not begun".into());
        }
        self.advance_batch_inner(input, count, true)
    }
    fn advance_batch_inner(
        &mut self,
        input: StepInput,
        count: usize,
        defer: bool,
    ) -> Result<(), String> {
        if self.field_pending {
            return Err("finish deferred LBM frame before advancing".into());
        }
        if count == 0 || count > 1024 {
            return Err("invalid lattice step count".into());
        }
        self.window.advance_batch(input, count)?;
        if defer {
            self.field_pending = true;
            Ok(())
        } else {
            self.refresh_field()
        }
    }
    /// Exactly one control per explicitly begun game frame, after its last batch.
    pub(crate) fn finish_frame(&mut self) -> Result<(), String> {
        if !self.frame_open || !self.field_pending {
            return Err("LBM frame finish requires one pending final batch".into());
        }
        let control = self.window.control_mean_density()?;
        self.density_control = Some(control);
        self.frame_open = false;
        self.refresh_field()
    }
    pub fn density_control(&self) -> Option<blood_fluid_gpu::lbm::MeanDensityControl> {
        self.density_control
    }
    fn refresh_field(&mut self) -> Result<(), String> {
        let field = self.window.field()?;
        self.window.diagnostics_from_field(&field)?;
        self.field = field;
        self.field_pending = false;
        if let Some(baseline) = &self.baseline {
            self.diagnostics.warmup.push(delta(baseline, &self.field));
            if self.diagnostics.warmup.len() >= WARMUP_STEPS {
                self.baseline = None;
            }
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn diagnostic_momentum(&mut self) -> Result<[f32; 2], String> {
        let field = self.window.field()?;
        let h = self.profile().cell_spacing();
        let mut total = [0.; 2];
        for (i, value) in field.iter().enumerate() {
            if !self.raster.geometry.solid[i] {
                total[0] += value[0] * value[1] * h * h;
                total[1] += value[0] * value[2] * h * h;
            }
        }
        Ok(total)
    }
    pub fn cached_diagnostics(
        &self,
    ) -> Result<(blood_fluid_gpu::lbm::Diagnostics, Vec<f32>), String> {
        if self.field_pending {
            return Err("LBM sampling cache awaits frame finish".into());
        }
        Ok((
            self.window.diagnostics_from_field(&self.field)?,
            self.window.opening_targets(),
        ))
    }
    pub fn work_stats(&self) -> blood_fluid_gpu::lbm::LbmWorkStats {
        self.window.work_stats()
    }
    pub fn profile(&self) -> GridProfile {
        self.window.spec().profile
    }
    pub fn lattice_dt(&self) -> f32 {
        self.dt
    }
    /// Row-major grid of lattice momentum changes (dimensionless density times lattice velocity).
    pub fn apply_kicks(&mut self, kicks: &[[f32; 2]]) -> Result<(), String> {
        self.window.apply_kicks(kicks)
    }
    /// Normalized fluid-only interpolation weights; also used to spread reaction momentum.
    pub fn supported_weights(
        &self,
        position: [f32; 2],
    ) -> Result<(LocalSampleStatus, Vec<(usize, f32)>), String> {
        if self.field_pending {
            return Err("LBM sampling cache awaits frame finish".into());
        }
        supported_weights(
            position,
            self.bounds()[0],
            self.profile(),
            &self.raster.geometry.solid,
            &self.field,
        )
    }
    pub fn sample(&self, position: [f32; 2]) -> Result<LocalSample, String> {
        let (status, weights) = self.supported_weights(position)?;
        let mut sample = LocalSample {
            density: 0.,
            velocity: [0.; 2],
            status,
        };
        for (i, w) in weights {
            sample.density += w * self.field[i][0];
            sample.velocity[0] += w * self.field[i][1];
            sample.velocity[1] += w * self.field[i][2];
        }
        Ok(sample)
    }
}
fn supported_weights(
    position: [f32; 2],
    origin: [f32; 2],
    profile: GridProfile,
    solid: &[bool],
    field: &[[f32; 4]],
) -> Result<(LocalSampleStatus, Vec<(usize, f32)>), String> {
    if !position.iter().all(|v| v.is_finite()) {
        return Err("nonfinite position".into());
    }
    let h = profile.cell_spacing();
    let n = profile.cells_per_side() as usize;
    let x = (position[0] - origin[0]) / h;
    let y = (position[1] - origin[1]) / h;
    if x < 0. || y < 0. || x >= n as f32 || y >= n as f32 {
        return Ok((LocalSampleStatus::OutsideWindow, vec![]));
    }
    let valid =
        |i: usize| !solid[i] && field[i][0] > 0. && field[i][..3].iter().all(|v| v.is_finite());
    if !valid(y.floor() as usize * n + x.floor() as usize) {
        return Ok((LocalSampleStatus::SolidOrInvalid, vec![]));
    }
    let x = x - 0.5;
    let y = y - 0.5;
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;
    let mut weights = Vec::with_capacity(4);
    for (dx, dy, w) in [
        (0, 0, (1. - fx) * (1. - fy)),
        (1, 0, fx * (1. - fy)),
        (0, 1, (1. - fx) * fy),
        (1, 1, fx * fy),
    ] {
        let xx = ix + dx;
        let yy = iy + dy;
        if w > 0. && xx >= 0 && yy >= 0 && xx < n as i32 && yy < n as i32 {
            let i = yy as usize * n + xx as usize;
            if valid(i) {
                weights.push((i, w));
            }
        }
    }
    let sum: f32 = weights.iter().map(|(_, w)| w).sum();
    if sum <= 0. {
        return Ok((LocalSampleStatus::SolidOrInvalid, vec![]));
    }
    for (_, w) in &mut weights {
        *w /= sum;
    }
    Ok((LocalSampleStatus::Valid, weights))
}
fn delta(baseline: &[(usize, [f32; 4])], field: &[[f32; 4]]) -> ContinuityDelta {
    let mut d = ContinuityDelta::default();
    for &(i, p) in baseline {
        let q = field[i];
        d.density = d.density.max((p[0] - q[0]).abs());
        d.velocity = d.velocity.max((p[1] - q[1]).hypot(p[2] - q[2]));
        d.momentum = d
            .momentum
            .max((p[0] * p[1] - q[0] * q[1]).hypot(p[0] * p[2] - q[0] * q[2]));
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streaming_window_block_alignment() {
        assert_eq!(
            origin_for([16., -0.01]).unwrap(),
            WindowOrigin {
                block_x: 0,
                block_y: -2
            }
        );
        assert_eq!(
            origin_for([15.99, 0.]).unwrap(),
            WindowOrigin {
                block_x: -1,
                block_y: -1
            }
        );
        assert!(origin_for([f32::NAN, 0.]).is_err());
    }
    #[test]
    fn supported_bilinear_weights_exclude_solids_and_outside() {
        let n = GridProfile::Coarse.cells_per_side() as usize;
        let mut solid = vec![false; n * n];
        let field = vec![[1., 0., 0., 0.]; n * n];
        let origin = [-16., 0.];
        let (status, weights) =
            supported_weights([-15.5, 0.5], origin, GridProfile::Coarse, &solid, &field).unwrap();
        assert_eq!(status, LocalSampleStatus::Valid);
        assert_eq!(weights.len(), 4);
        assert!(weights.iter().all(|(_, w)| (*w - 0.25).abs() < 1e-6));
        solid[n] = true;
        let (_, weights) =
            supported_weights([-15.5, 0.5], origin, GridProfile::Coarse, &solid, &field).unwrap();
        assert_eq!(weights.len(), 3);
        assert!((weights.iter().map(|(_, w)| w).sum::<f32>() - 1.).abs() < 1e-6);
        assert!(weights.iter().all(|(i, _)| *i != n));
        assert_eq!(
            supported_weights([-15.75, 0.75], origin, GridProfile::Coarse, &solid, &field)
                .unwrap()
                .0,
            LocalSampleStatus::SolidOrInvalid
        );
        assert_eq!(
            supported_weights([-16.01, 0.5], origin, GridProfile::Coarse, &solid, &field)
                .unwrap()
                .0,
            LocalSampleStatus::OutsideWindow
        );
    }

    #[test]
    #[ignore = "explicit software-adapter functional diagnostic"]
    fn invalid_gpu_macrostate_returns_error_from_batch() {
        let (map, flow) = fixture(false);
        let mut window = StreamingWindow::new(
            &map,
            &flow,
            [24., 24.],
            GridProfile::Coarse,
            WindowConfig {
                dt: 0.01,
                viscosity: 0.1,
                allow_software: true,
            },
        )
        .unwrap();
        let n = GridProfile::Coarse.cells_per_side() as usize;
        let mut kicks = vec![[0., 0.]; n * n];
        kicks[48 * n + 48] = [f32::MAX, f32::MAX];
        window.apply_kicks(&kicks).unwrap();
        let error = window
            .advance_batch(
                StepInput {
                    acceleration: [0., 0.],
                },
                1,
            )
            .unwrap_err();
        assert!(error.contains("invalid fluid state"), "{error}");
    }

    #[test]
    #[ignore = "explicit software-adapter functional diagnostic"]
    fn batched_advance_matches_individual_steps() {
        let (map, flow) = fixture(false);
        let config = WindowConfig {
            dt: 0.01,
            viscosity: 0.1,
            allow_software: true,
        };
        let mut a =
            StreamingWindow::new(&map, &flow, [24., 24.], GridProfile::Coarse, config).unwrap();
        let mut b =
            StreamingWindow::new(&map, &flow, [24., 24.], GridProfile::Coarse, config).unwrap();
        let n = GridProfile::Coarse.cells_per_side() as usize;
        for (i, velocity) in [
            (48 * n + 48, 1.),
            (48 * n + 49, 3.),
            (49 * n + 48, 5.),
            (49 * n + 49, 7.),
        ] {
            a.field[i][1] = velocity;
        }
        let sample = a.sample([24.5, 24.5]).unwrap();
        assert!((sample.velocity[0] - 4.).abs() < 1e-6);
        // Restore cache from the unchanged GPU before comparing time advancement.
        a.field = a.window.field().unwrap();
        for (count, acceleration) in [(4, [0.015, -0.02]), (3, [-0.01, 0.012])] {
            let input = StepInput { acceleration };
            a.window.reset_work_stats();
            a.advance_batch(input, count).unwrap();
            // One lattice batch submission, then the existing macro-field read.
            assert_eq!(a.work_stats().submissions, 2);
            assert_eq!(a.work_stats().lattice_steps, count as u64);
            for _ in 0..count {
                b.advance(input).unwrap();
            }
            for (x, y) in a.field.iter().zip(&b.field) {
                for (x, y) in x.iter().zip(y) {
                    assert!((x - y).abs() < 1e-6);
                }
            }
        }
    }
    #[test]
    #[ignore = "explicit software-adapter functional diagnostic"]
    fn deferred_batch_requires_finish_and_refreshes_cache() {
        let (map, flow) = fixture(false);
        let mut window = StreamingWindow::new(
            &map,
            &flow,
            [24., 24.],
            GridProfile::Coarse,
            WindowConfig {
                dt: 0.01,
                viscosity: 0.1,
                allow_software: true,
            },
        )
        .unwrap();
        assert!(window.finish_frame().is_err());
        window.begin_frame();
        window
            .advance_batch_deferred(
                StepInput {
                    acceleration: [0., 0.],
                },
                2,
            )
            .unwrap();
        assert!(window.sample([24., 24.]).is_err());
        assert!(window.cached_diagnostics().is_err());
        assert!(window.maybe_shift(&map, &flow, [40., 24.]).is_err());
        window.finish_frame().unwrap();
        assert_eq!(window.work_stats().readback_calls, 2);
        assert!(window.sample([24., 24.]).unwrap().density > 0.);
        assert_eq!(window.field, window.window.field().unwrap());
        assert!(window.finish_frame().is_err());
        window.begin_frame();
        assert!(window.maybe_shift(&map, &flow, [40., 24.]).unwrap());
        window
            .advance_batch_deferred(
                StepInput {
                    acceleration: [0., 0.],
                },
                2,
            )
            .unwrap();
        window.finish_frame().unwrap();
        assert_eq!(window.field, window.window.field().unwrap());
        assert!(window.density_control().is_some());
        let (d, targets) = window.cached_diagnostics().unwrap();
        for (q, t) in d.opening_fluxes.iter().zip(targets) {
            assert!((q - t).abs() < 1e-4);
        }
    }
    fn fixture(fork: bool) -> (MapDefinition, GraphFlow) {
        use crate::{circulation::Flow, map::VesselEdge};
        let nodes = if fork {
            vec![[-50., 24.], [32., 24.], [100., 24.], [32., 100.]]
        } else {
            vec![[-50., 24.], [100., 24.]]
        };
        let tuples = if fork {
            vec![(0, 1, 3.), (1, 2, 2.), (1, 3, 1.)]
        } else {
            vec![(0, 1, 3.)]
        };
        let edges = tuples
            .into_iter()
            .map(|(from, to, half_width)| VesselEdge {
                from,
                to,
                half_width,
                centerline: vec![nodes[from], nodes[to]],
            })
            .collect();
        (
            MapDefinition {
                fluid: None,
                nodes,
                edges,
                pump_edge: 0,
                spawn: [0., 0.],
                landmarks: vec![],
            },
            GraphFlow {
                pressures: vec![],
                rates: if fork { vec![0.6, 0.4, 0.2] } else { vec![0.6] },
                summary: Flow {
                    pressure_drop: 0.,
                    resistance: 0.,
                    rate: 0.,
                    mean_speed: 0.,
                },
            },
        )
    }
    #[test]
    #[ignore = "explicit software-adapter functional diagnostic"]
    fn streaming_window_migration_reference() {
        let config = WindowConfig {
            dt: 0.01,
            viscosity: 0.1,
            allow_software: true,
        };
        for fork in [false, true] {
            let (map, mut flow) = fixture(fork);
            let mut moving =
                StreamingWindow::new(&map, &flow, [24., 24.], GridProfile::Coarse, config).unwrap();
            let mut fixed =
                StreamingWindow::new(&map, &flow, [24., 24.], GridProfile::Coarse, config).unwrap();
            for _ in 0..4 {
                moving
                    .advance(StepInput {
                        acceleration: [0., 0.],
                    })
                    .unwrap();
                fixed
                    .advance(StepInput {
                        acceleration: [0., 0.],
                    })
                    .unwrap();
            }
            for rate in &mut flow.rates {
                *rate *= 0.5;
            }
            moving.begin_frame();
            fixed.begin_frame();
            assert!(moving.maybe_shift(&map, &flow, [40., 24.]).unwrap());
            assert!(!moving.maybe_shift(&map, &flow, [56., 24.]).unwrap());
            assert_eq!(
                moving.active_origin(),
                WindowOrigin {
                    block_x: 1,
                    block_y: 0
                }
            );
            fixed.maybe_shift(&map, &flow, [24., 24.]).unwrap();
            assert!(moving.diagnostics.overlap_error < 1e-6);
            if !fork {
                let exposed = moving.sample([60.25, 24.25]).unwrap();
                let expected = 3.0 * 0.3 / 12.0 * (1.0 - (0.25_f32 / 3.0).powi(2));
                assert!((exposed.velocity[0] - expected).abs() < 1e-5);
            }
            let mut max = ContinuityDelta::default();
            for _ in 0..8 {
                moving
                    .advance(StepInput {
                        acceleration: [0., 0.],
                    })
                    .unwrap();
                fixed
                    .advance(StepInput {
                        acceleration: [0., 0.],
                    })
                    .unwrap();
                for y in 40..56 {
                    for x in 64..88 {
                        let p = [x as f32 * 0.5 + 0.25, y as f32 * 0.5 + 0.25];
                        let a = moving.sample(p).unwrap();
                        let b = fixed.sample(p).unwrap();
                        if a.status == LocalSampleStatus::Valid
                            && b.status == LocalSampleStatus::Valid
                        {
                            max.density = max.density.max((a.density - b.density).abs());
                            max.velocity = max.velocity.max(
                                (a.velocity[0] - b.velocity[0])
                                    .hypot(a.velocity[1] - b.velocity[1]),
                            );
                            max.momentum = max.momentum.max(
                                (a.density * a.velocity[0] - b.density * b.velocity[0])
                                    .hypot(a.density * a.velocity[1] - b.density * b.velocity[1]),
                            );
                        }
                    }
                }
            }
            eprintln!(
                "fork={fork} migration vs fixed: {max:?}, warmup {:?}",
                moving.diagnostics.warmup
            );
            assert!(max.density < 0.02 && max.velocity < 0.05 && max.momentum < 0.05);
            assert_eq!(moving.diagnostics.warmup.len(), 8);
        }
    }
}
