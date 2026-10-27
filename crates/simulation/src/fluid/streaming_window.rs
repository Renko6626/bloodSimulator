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
    shifted: bool,
    baseline: Option<Vec<(usize, [f32; 4])>>,
    pub diagnostics: MigrationDiagnostics,
}

fn origin_for(position: [f32; 2]) -> Result<WindowOrigin, String> {
    if !position.iter().all(|v| {
        v.is_finite() && *v >= (i32::MIN as f32 + 2.) * 16. && *v < (i32::MAX as f32 - 2.) * 16.
    }) {
        return Err("invalid player position".into());
    }
    Ok(WindowOrigin {
        block_x: (position[0] / 16.).floor() as i32 - 1,
        block_y: (position[1] / 16.).floor() as i32 - 1,
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
            shifted: false,
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
        self.shifted = false;
    }
    /// Update current targets even when no migration is required.
    pub fn maybe_shift(
        &mut self,
        map: &MapDefinition,
        flow: &GraphFlow,
        position: [f32; 2],
    ) -> Result<bool, String> {
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
        self.window.advance(input)?;
        self.field = self.window.field()?;
        if let Some(baseline) = &self.baseline {
            self.diagnostics.warmup.push(delta(baseline, &self.field));
            if self.diagnostics.warmup.len() >= WARMUP_STEPS {
                self.baseline = None;
            }
        }
        Ok(())
    }
    pub fn sample(&self, position: [f32; 2]) -> Result<LocalSample, String> {
        if !position.iter().all(|v| v.is_finite()) {
            return Err("nonfinite position".into());
        }
        let o = self.bounds()[0];
        let h = self.window.spec().profile.cell_spacing();
        let n = self.window.spec().profile.cells_per_side() as usize;
        let x = ((position[0] - o[0]) / h).floor() as i64;
        let y = ((position[1] - o[1]) / h).floor() as i64;
        if x < 0 || y < 0 || x >= n as i64 || y >= n as i64 {
            return Ok(LocalSample {
                density: 0.,
                velocity: [0.; 2],
                status: LocalSampleStatus::OutsideWindow,
            });
        }
        let i = y as usize * n + x as usize;
        let p = self.field[i];
        let valid =
            !self.raster.geometry.solid[i] && p[0] > 0. && p[..3].iter().all(|v| v.is_finite());
        Ok(LocalSample {
            density: if valid { p[0] } else { 0. },
            velocity: if valid { [p[1], p[2]] } else { [0.; 2] },
            status: if valid {
                LocalSampleStatus::Valid
            } else {
                LocalSampleStatus::SolidOrInvalid
            },
        })
    }
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
