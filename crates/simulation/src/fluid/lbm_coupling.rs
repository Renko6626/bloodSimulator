//! Finite-resistance exchange with the local LBM window in world coordinates.
use super::streaming_window::StreamingWindow;
use crate::{Config, circulation::graph::GraphFlow, map::MapDefinition};
use blood_fluid_gpu::lbm::{GridProfile, LocalSampleStatus, StepInput, WindowConfig};
use rapier2d::prelude::*;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct LbmOptions {
    pub profile: GridProfile,
    pub outer_band: f32,
    /// Upper bound on world fluid speed used by the lattice stability selection.
    pub velocity_bound: f32,
    /// Only for explicit functional diagnostics.
    pub allow_software: bool,
}
impl Default for LbmOptions {
    fn default() -> Self {
        Self {
            profile: GridProfile::Coarse,
            outer_band: 4.,
            velocity_bound: 10.,
            allow_software: false,
        }
    }
}
#[derive(Clone, Debug, Default)]
pub struct LbmStats {
    pub lattice_steps_per_substep: usize,
    pub lattice_dt: f32,
    pub elapsed_fluid_time: f64,
    pub coupling_ms: f64,
    pub lbm_ms: f64,
    pub rapier_ms: f64,
}
pub(crate) struct LbmCoupling {
    pub window: StreamingWindow,
    pub options: LbmOptions,
    pub stats: LbmStats,
}
impl LbmCoupling {
    pub fn new(
        map: &MapDefinition,
        flow: &GraphFlow,
        config: &Config,
        options: LbmOptions,
    ) -> Result<Self, String> {
        let dt = config.dt / config.substeps as f32;
        let h = options.profile.cell_spacing();
        if !dt.is_finite()
            || dt <= 0.
            || config.substeps == 0
            || !config.viscosity.is_finite()
            || config.viscosity <= 0.
            || !options.velocity_bound.is_finite()
            || options.velocity_bound <= 0.
            || !options.outer_band.is_finite()
            || options.outer_band <= 0.
            || options.outer_band > 24.
        {
            return Err("invalid LBM time, viscosity, velocity bound or outer band".into());
        }
        let limit = (0.05 * h / options.velocity_bound).min(0.5 * h * h / config.viscosity);
        let count = (dt / limit).ceil().max(1.) as usize;
        if count > 1024 {
            return Err("LBM lattice step count exceeds 1024".into());
        }
        let lattice_dt = dt / count as f32;
        let window = StreamingWindow::new(
            map,
            flow,
            map.spawn,
            options.profile,
            WindowConfig {
                dt: lattice_dt,
                viscosity: config.viscosity,
                allow_software: options.allow_software,
            },
        )?;
        Ok(Self {
            window,
            options,
            stats: LbmStats {
                lattice_steps_per_substep: count,
                lattice_dt,
                ..Default::default()
            },
        })
    }
    pub fn prepare(
        &mut self,
        map: &MapDefinition,
        flow: &GraphFlow,
        position: Vector,
    ) -> Result<(), String> {
        self.window.maybe_shift(map, flow, position.to_array())?;
        Ok(())
    }
    /// Returns the supported local weight and local velocity; solids have neither drive.
    pub fn local(&self, map: &MapDefinition, p: Vector) -> (f32, Vector, bool) {
        if !p.is_finite() || map.signed_distance(p) >= 0. {
            return (0., Vector::ZERO, false);
        }
        let Ok(sample) = self.window.sample(p.to_array()) else {
            return (0., Vector::ZERO, false);
        };
        match sample.status {
            LocalSampleStatus::OutsideWindow => (0., Vector::ZERO, true),
            LocalSampleStatus::SolidOrInvalid => (0., Vector::ZERO, false),
            LocalSampleStatus::Valid => {
                let [min, max] = self.window.bounds();
                let d = (p.x - min[0])
                    .min(max[0] - p.x)
                    .min(p.y - min[1])
                    .min(max[1] - p.y);
                let x = (d / self.options.outer_band).clamp(0., 1.);
                (
                    x * x * (3. - 2. * x),
                    Vector::from_array(sample.velocity),
                    true,
                )
            }
        }
    }
    pub fn advance(
        &mut self,
        map: &MapDefinition,
        world: &mut PhysicsWorld,
        cells: &[SoftBodyHandle],
        robot: RigidBodyHandle,
        dt: f32,
        drag: f32,
    ) -> Result<(), String> {
        let started = Instant::now();
        let kicks = self.exchange(map, world, cells, robot, dt, drag)?;
        self.stats.coupling_ms += started.elapsed().as_secs_f64() * 1000.;
        let started = Instant::now();
        self.window.apply_kicks(&kicks)?;
        self.window.advance_batch(
            StepInput {
                acceleration: [0., 0.],
            },
            self.stats.lattice_steps_per_substep,
        )?;
        self.stats.lbm_ms += started.elapsed().as_secs_f64() * 1000.;
        self.stats.elapsed_fluid_time +=
            self.stats.lattice_dt as f64 * self.stats.lattice_steps_per_substep as f64;
        Ok(())
    }
    pub(crate) fn exchange(
        &self,
        map: &MapDefinition,
        world: &mut PhysicsWorld,
        cells: &[SoftBodyHandle],
        robot: RigidBodyHandle,
        dt: f32,
        drag: f32,
    ) -> Result<Vec<[f32; 2]>, String> {
        let n = self.window.profile().cells_per_side() as usize;
        let mut kicks = vec![[0.; 2]; n * n];
        let h = self.window.profile().cell_spacing();
        let conversion = self.window.lattice_dt() / (h * h * h);
        // Collect every marker before any impulses are applied.
        let mut markers = Vec::new();
        for &handle in cells {
            let b = &world.soft_bodies[handle];
            let count = b.num_particles();
            let center = b.particle_positions().sum::<Vector>() / count as f32;
            let mean = b.particle_velocities().sum::<Vector>() / count as f32;
            for k in 0..4 {
                let node = k * count / 4;
                markers.push((
                    Some((handle, node)),
                    (b.particle_position(node) + center) * 0.5,
                    (b.particle_velocity(node) + mean) * 0.5,
                ));
            }
        }
        let b = &world.bodies[robot];
        for offset in [Vector::X, Vector::Y, -Vector::X, -Vector::Y] {
            markers.push((None, b.translation() + offset * 0.425, b.linvel()));
        }
        for (owner, p, v) in markers {
            let (weight, u, valid) = self.local(map, p);
            if !valid || weight == 0. {
                continue;
            }
            let (_, weights) = self.window.supported_weights(p.to_array())?;
            let impulse = (u - v) * (0.25 / 4. * (drag / 20.) * dt * weight);
            if let Some((handle, node)) = owner {
                let b = &mut world.soft_bodies[handle];
                let count = b.num_particles();
                for i in 0..count {
                    b.apply_particle_impulse(
                        i,
                        impulse * (0.5 / count as f32 + if i == node { 0.5 } else { 0. }),
                        true,
                    );
                }
            } else {
                world.bodies[robot].apply_impulse(impulse, true);
            }
            for (i, w) in weights {
                kicks[i][0] -= impulse.x * w * conversion;
                kicks[i][1] -= impulse.y * w * conversion;
            }
        }
        Ok(kicks)
    }
}
