//! Bounded fixed-domain averaging. The report describes the budget, not convergence.
use super::{
    Diagnostics, DomainSpec, InitialField, LbmDomain, LbmResult, StepInput, WindowConfig,
    WindowGeometry,
};
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct BakeSchedule {
    pub steps_per_frame: usize,
    pub warmup_frames: usize,
    pub sample_frames: usize,
    pub sample_stride: usize,
}
impl BakeSchedule {
    pub fn sample_count(self) -> LbmResult<usize> {
        if !(1..=1024).contains(&self.steps_per_frame) || self.sample_stride == 0 {
            return Err("bake requires 1..=1024 steps per frame and positive sample stride".into());
        }
        self.warmup_frames
            .checked_add(self.sample_frames)
            .and_then(|frames| frames.checked_mul(self.steps_per_frame))
            .ok_or("bake frame/step budget overflow")?;
        let count = self.sample_frames / self.sample_stride;
        if count < 2 {
            return Err("bake requires at least two samples for half-mean diagnostics".into());
        }
        Ok(count)
    }
}

#[derive(Clone, Debug)]
pub struct BakeReport {
    pub schedule: BakeSchedule,
    pub lattice_dt: f32,
    pub samples: usize,
    pub warmup_simulated_seconds: f64,
    pub sample_simulated_seconds: f64,
    pub wall_seconds: f64,
    /// Extrema over sampled, post-control fluid states.
    pub density_min: f32,
    pub density_max: f32,
    pub mean_opening_fluxes: Vec<f64>,
    /// Magnitude of the difference between first/second half sample means.
    pub half_mean_velocity_rms: f64,
    pub half_mean_velocity_max: f64,
    pub final_diagnostics: Diagnostics,
}
#[derive(Clone, Debug)]
pub struct BakedFlow {
    pub domain: DomainSpec,
    pub viscosity: f32,
    pub velocity: Vec<[f32; 2]>,
    pub fluid: Vec<bool>,
    pub report: BakeReport,
}

/// Samples are taken after completed stride frames; trailing partial strides are not sampled.
pub fn bake_mean(
    domain: DomainSpec,
    config: WindowConfig,
    initial: InitialField,
    geometry: WindowGeometry,
    schedule: BakeSchedule,
) -> LbmResult<BakedFlow> {
    let samples = schedule.sample_count()?;
    let started = Instant::now();
    eprintln!(
        "offline LBM: preparing {}² domain, h={}, dt={}",
        domain.cells_per_side, domain.spacing, config.dt
    );
    let mut solver = LbmDomain::new(domain, config, initial, geometry)?;
    eprintln!(
        "offline LBM: solver ready after {:.3}s",
        started.elapsed().as_secs_f64()
    );
    let fluid: Vec<bool> = solver.geometry().solid.iter().map(|s| !s).collect();
    let mut mean = MeanAccumulator::new(&fluid, samples);
    let mut density_min = f32::INFINITY;
    let mut density_max = f32::NEG_INFINITY;
    let mut flux_sum = vec![0_f64; solver.geometry().openings.len()];
    for frame in 0..schedule.warmup_frames {
        solver.advance_batch(
            StepInput {
                acceleration: [0.; 2],
            },
            schedule.steps_per_frame,
        )?;
        solver.control_mean_density()?;
        if (frame + 1) % (schedule.warmup_frames / 10).max(1) == 0
            || frame + 1 == schedule.warmup_frames
        {
            eprintln!(
                "offline LBM: warmup {}/{}; elapsed {:.3}s",
                frame + 1,
                schedule.warmup_frames,
                started.elapsed().as_secs_f64()
            );
        }
    }
    for frame in 0..schedule.sample_frames {
        solver.advance_batch(
            StepInput {
                acceleration: [0.; 2],
            },
            schedule.steps_per_frame,
        )?;
        solver.control_mean_density()?;
        if (frame + 1) % (schedule.sample_frames / 10).max(1) == 0
            || frame + 1 == schedule.sample_frames
        {
            eprintln!(
                "offline LBM: sampling {}/{}; elapsed {:.3}s",
                frame + 1,
                schedule.sample_frames,
                started.elapsed().as_secs_f64()
            );
        }
        if (frame + 1) % schedule.sample_stride != 0 {
            continue;
        }
        let field = solver.field()?;
        let diagnostics = solver.diagnostics_from_field(&field)?;
        validate_diagnostics(&diagnostics)?;
        mean.add(&field)?;
        density_min = density_min.min(diagnostics.density_min);
        density_max = density_max.max(diagnostics.density_max);
        for (sum, flux) in flux_sum.iter_mut().zip(diagnostics.opening_fluxes) {
            *sum += f64::from(flux);
        }
    }
    let (velocity, rms, max) = mean.finish()?;
    let final_diagnostics = solver.snapshot_diagnostics()?;
    validate_diagnostics(&final_diagnostics)?;
    let frame_seconds = f64::from(config.dt) * schedule.steps_per_frame as f64;
    Ok(BakedFlow {
        domain,
        viscosity: config.viscosity,
        velocity,
        fluid,
        report: BakeReport {
            schedule,
            lattice_dt: config.dt,
            samples,
            warmup_simulated_seconds: frame_seconds * schedule.warmup_frames as f64,
            sample_simulated_seconds: frame_seconds * schedule.sample_frames as f64,
            wall_seconds: started.elapsed().as_secs_f64(),
            density_min,
            density_max,
            mean_opening_fluxes: flux_sum.into_iter().map(|f| f / samples as f64).collect(),
            half_mean_velocity_rms: rms,
            half_mean_velocity_max: max,
            final_diagnostics,
        },
    })
}
fn validate_diagnostics(d: &Diagnostics) -> LbmResult<()> {
    if [d.density_min, d.density_max, d.max_speed, d.mass]
        .iter()
        .chain(d.opening_fluxes.iter())
        .any(|v| !v.is_finite())
    {
        return Err("nonfinite bake diagnostics".into());
    }
    Ok(())
}

// CPU-only arithmetic shared by the bake and focused tests. Solid samples export zero.
struct MeanAccumulator<'a> {
    fluid: &'a [bool],
    expected: usize,
    count: usize,
    first: Vec<[f64; 2]>,
    second: Vec<[f64; 2]>,
}
impl<'a> MeanAccumulator<'a> {
    fn new(fluid: &'a [bool], expected: usize) -> Self {
        Self {
            fluid,
            expected,
            count: 0,
            first: vec![[0.; 2]; fluid.len()],
            second: vec![[0.; 2]; fluid.len()],
        }
    }
    fn add(&mut self, field: &[[f32; 4]]) -> LbmResult<()> {
        if field.len() != self.fluid.len() || self.count >= self.expected {
            return Err("bake sample length/count mismatch".into());
        }
        // Check before modifying accumulators so errors cannot become a partial sample.
        for (cell, fluid) in field.iter().zip(self.fluid) {
            if *fluid && (cell[0] <= 0. || cell[..3].iter().any(|v| !v.is_finite())) {
                return Err("invalid bake sample".into());
            }
        }
        let sums = if self.count < self.expected / 2 {
            &mut self.first
        } else {
            &mut self.second
        };
        for ((sum, cell), fluid) in sums.iter_mut().zip(field).zip(self.fluid) {
            if *fluid {
                for k in 0..2 {
                    sum[k] += f64::from(cell[k + 1]);
                }
            }
        }
        self.count += 1;
        Ok(())
    }
    fn finish(self) -> LbmResult<(Vec<[f32; 2]>, f64, f64)> {
        if self.count != self.expected || self.expected < 2 {
            return Err("incomplete bake samples".into());
        }
        let n_first = self.expected / 2;
        let n_second = self.expected - n_first;
        let mut squared = 0.;
        let mut max = 0_f64;
        let mut cells = 0;
        let velocity = self
            .first
            .iter()
            .zip(&self.second)
            .zip(self.fluid)
            .map(|((a, b), fluid)| {
                if *fluid {
                    cells += 1;
                    let dx = a[0] / n_first as f64 - b[0] / n_second as f64;
                    let dy = a[1] / n_first as f64 - b[1] / n_second as f64;
                    let change = dx.hypot(dy);
                    squared += change * change;
                    max = max.max(change);
                }
                [
                    ((a[0] + b[0]) / self.expected as f64) as f32,
                    ((a[1] + b[1]) / self.expected as f64) as f32,
                ]
            })
            .collect::<Vec<_>>();
        if cells == 0 || velocity.iter().flatten().any(|v| !v.is_finite()) {
            return Err("invalid bake mean".into());
        }
        Ok((velocity, (squared / cells as f64).sqrt(), max))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn averaging_uses_actual_samples_and_separate_unequal_halves() {
        let fluid = [true, false];
        let mut mean = MeanAccumulator::new(&fluid, 3);
        for x in [1., 3., 5.] {
            mean.add(&[[1., x, 2. * x, 0.], [0., 99., 99., 0.]])
                .unwrap();
        }
        let (velocity, rms, max) = mean.finish().unwrap();
        assert_eq!(velocity, [[3., 6.], [0., 0.]]);
        assert!((rms - 45_f64.sqrt()).abs() < 1e-12);
        assert_eq!(rms, max);
    }
    #[test]
    fn budgets_and_invalid_or_missing_samples_are_rejected() {
        let schedule = BakeSchedule {
            steps_per_frame: 14,
            warmup_frames: 2,
            sample_frames: 11,
            sample_stride: 3,
        };
        assert_eq!(schedule.sample_count().unwrap(), 3);
        assert!(
            BakeSchedule {
                sample_stride: 0,
                ..schedule
            }
            .sample_count()
            .is_err()
        );
        assert!(
            BakeSchedule {
                sample_frames: 5,
                ..schedule
            }
            .sample_count()
            .is_err()
        );
        assert!(
            BakeSchedule {
                warmup_frames: usize::MAX,
                ..schedule
            }
            .sample_count()
            .is_err()
        );
        let mut mean = MeanAccumulator::new(&[true], 2);
        assert!(mean.add(&[[1., f32::NAN, 0., 0.]]).is_err());
        mean.add(&[[1., 2., 0., 0.]]).unwrap();
        assert!(mean.finish().is_err());
    }
}
