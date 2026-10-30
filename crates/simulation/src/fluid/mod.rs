//! World-space local fluid fields. Owns fields and marker-force history, never
//! integrates a body: the caller keeps the sole Rapier stepping location.
mod grid;
#[cfg(feature = "lbm-gpu")]
pub mod lbm_coupling;
#[cfg(feature = "lbm-gpu")]
pub mod lbm_geometry;
#[cfg(feature = "lbm-gpu")]
pub mod streaming_window;

mod regions;
pub(crate) use regions::LocalRegions;
#[cfg(test)]
mod tests;

use crate::{circulation::graph::GraphFlow, map::MapDefinition};
use grid::{Field, Grid};
use rapier2d::prelude::*;
use std::time::Instant;

#[derive(Clone)]
pub struct LocalFluidOptions {
    pub n: usize,
    pub h: f64,
    pub origins: [[f64; 2]; 2],
    pub solver: std::sync::Arc<dyn blood_fluid::SolverFactory>,
}
impl Default for LocalFluidOptions {
    fn default() -> Self {
        Self {
            n: 64,
            h: 0.25,
            origins: [[-22., -8.], [-18., -16.]],
            solver: std::sync::Arc::new(blood_fluid::cpu::LuFactory),
        }
    }
}
const OMEGA: f64 = 0.5;

#[derive(Clone, Debug, Default)]
pub struct FluidStats {
    pub bounds: [[f32; 2]; 2],
    pub active_cells: usize,
    pub solve_ms: f64,
    pub divergence_rms: f64,
    pub momentum_rms: f64,
    pub net_flux: f64,
    pub exchange_error: f64,
    pub pressure_iterations: usize,
    pub region_switches: usize,
    pub handoff_ms: f64,
    pub handoff_velocity_delta: f32,
}

pub(crate) struct LocalFluid {
    grid: Grid,
    background: Field,
    background_rhs: Vec<f64>,
    feedback: Field,
    combined: Field,
    history: Vec<[[f64; 2]; 4]>,
    pressure: f64,
    pub stats: FluidStats,
}

struct Marker {
    owner: usize,
    selected_node: Option<usize>,
    weights: [Vec<(usize, f64)>; 2],
    force: [f64; 2],
}

impl LocalFluid {
    fn new_at(
        map: &MapDefinition,
        unit_flow: &GraphFlow,
        viscosity: f32,
        cells: usize,
        origin: [f64; 2],
        options: &LocalFluidOptions,
    ) -> Result<Self, String> {
        let n = options.n;
        let h = options.h;
        let mask = (0..n * n)
            .map(|k| {
                map.signed_distance(Vector::new(
                    (origin[0] + (k % n) as f64 * h + h / 2.0) as f32,
                    (origin[1] + (k / n) as f64 * h + h / 2.0) as f32,
                )) < -0.08
            })
            .collect();
        let mut grid =
            Grid::with_factory(n, h, origin, mask, 1.0, [true; 4], options.solver.as_ref())?;
        let boundary = |position: [f64; 2]| {
            let p = Vector::new(position[0] as f32, position[1] as f32);
            let edge = map.nearest_edge(p);
            let projection = map.project_edge(edge, p);
            let t = (projection.along / map.edge_length(edge)).clamp(0.0, 1.0) as f64;
            let e = &map.edges[edge];
            let graph_pressure = (1.0 - t) * unit_flow.pressures[e.from] as f64
                + t * unit_flow.pressures[e.to] as f64;
            // Match planar/circular straight-pipe mean speed, not the viscosity value.
            3.0 / (8.0 * viscosity as f64) * graph_pressure
        };
        let background_rhs = grid.boundary_rhs(&boundary);
        let background = grid.solve(&vec![0.0; grid.velocity_count()], &boundary)?;
        if !background.divergence_rms.is_finite()
            || !background.momentum_rms.is_finite()
            || !background.net_flux.is_finite()
        {
            return Err("non-finite local background field".into());
        }
        let zero = || Field {
            velocity: vec![0.0; grid.velocity_count()],
            pressure: vec![0.0; grid.cells.len()],
            divergence_rms: 0.0,
            momentum_rms: 0.0,
            net_flux: 0.0,
            iterations: 0,
        };
        let feedback = zero();
        let combined = zero();
        Ok(Self {
            grid,
            background,
            background_rhs,
            feedback,
            combined,
            history: vec![[[0.0; 2]; 4]; cells + 1],
            pressure: 0.0,
            stats: FluidStats {
                bounds: [
                    [origin[0] as f32, origin[1] as f32],
                    [
                        (origin[0] + n as f64 * h) as f32,
                        (origin[1] + n as f64 * h) as f32,
                    ],
                ],
                ..FluidStats::default()
            },
        })
    }

    pub fn begin_frame(&mut self) {
        self.stats.solve_ms = 0.0;
        self.stats.pressure_iterations = 0;
        self.stats.exchange_error = 0.0;
        self.stats.region_switches = 0;
        self.stats.handoff_ms = 0.0;
        self.stats.handoff_velocity_delta = 0.0;
    }

    fn clearance(&self, position: Vector) -> f32 {
        let [min, max] = self.stats.bounds;
        (position.x - min[0])
            .min(max[0] - position.x)
            .min(position.y - min[1])
            .min(max[1] - position.y)
    }

    /// Full support and a one-unit ramp before enabling the weak local coupling.
    pub fn weight(&self, position: Vector, radius: f32) -> f32 {
        let x = ((self.clearance(position) - radius - 0.5) / 1.0).clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    }

    pub fn sample(&self, position: Vector) -> Vector {
        let value = self
            .grid
            .sample(&self.combined, [position.x as f64, position.y as f64]);
        Vector::new(value[0] as f32, value[1] as f32)
    }

    pub fn prepare_pressure(&mut self, pressure: f32) {
        self.pressure = pressure as f64;
        for (out, (base, feedback)) in self
            .combined
            .velocity
            .iter_mut()
            .zip(self.background.velocity.iter().zip(&self.feedback.velocity))
        {
            *out = base * self.pressure + feedback;
        }
    }

    pub fn positions(&self) -> impl Iterator<Item = Vector> + '_ {
        let n = self.grid.n;
        self.grid
            .cells
            .iter()
            .filter(move |&&k| k % n % 6 == 3 && k / n % 6 == 3)
            .map(move |&k| {
                Vector::new(
                    (self.grid.origin[0] + (k % n) as f64 * self.grid.h + self.grid.h / 2.0) as f32,
                    (self.grid.origin[1] + (k / n) as f64 * self.grid.h + self.grid.h / 2.0) as f32,
                )
            })
    }

    fn collect_markers(
        &mut self,
        world: &PhysicsWorld,
        cells: &[SoftBodyHandle],
        robot: RigidBodyHandle,
        drag: f32,
        _radius: f32,
        update: bool,
    ) -> Result<(Vec<Marker>, usize), String> {
        if self.history.len() != cells.len() + 1 {
            self.history.resize(cells.len() + 1, [[0.0; 2]; 4]);
            self.history.fill([[0.0; 2]; 4]);
        }
        let mut markers = Vec::new();
        let mut active = 0;
        for owner in 0..=cells.len() {
            let (center, mean_velocity, count) = if owner < cells.len() {
                let body = &world.soft_bodies[cells[owner]];
                let count = body.num_particles();
                (
                    body.particle_positions().sum::<Vector>() / count as f32,
                    body.particle_velocities().sum::<Vector>() / count as f32,
                    count,
                )
            } else {
                let b = &world.bodies[robot];
                (b.translation(), b.linvel(), 0)
            };
            let body_size = if count == 0 {
                0.85
            } else {
                body_radius(&world.soft_bodies[cells[owner]])
            };
            let weight = self.weight(center, body_size) as f64;
            if weight == 0.0 || drag == 0.0 {
                if update {
                    self.history[owner] = [[0.0; 2]; 4];
                }
                continue;
            }
            if count > 0 {
                active += 1;
            }
            for k in 0..4 {
                let (position, velocity, node) = if count > 0 {
                    let j = k * count / 4;
                    let b = &world.soft_bodies[cells[owner]];
                    (
                        (b.particle_position(j) + center) * 0.5,
                        (b.particle_velocity(j) + mean_velocity) * 0.5,
                        Some(j),
                    )
                } else {
                    let offset = [Vector::X, Vector::Y, -Vector::X, -Vector::Y][k] * 0.425;
                    (center + offset, mean_velocity, None)
                };
                let weights = std::array::from_fn(|axis| {
                    self.grid
                        .weights([position.x as f64, position.y as f64], axis)
                });
                if weights.iter().any(|w| w.is_empty()) {
                    return Err("local marker has no fluid support".into());
                }
                let force = if update {
                    std::array::from_fn(|axis| {
                        let sample = weights[axis]
                            .iter()
                            .map(|&(i, w)| w * self.combined.velocity[i])
                            .sum::<f64>();
                        let trial = (0.25 / 4.0)
                            * (drag as f64 / 20.0)
                            * weight
                            * (velocity[axis] as f64 - sample);
                        (1.0 - OMEGA) * self.history[owner][k][axis] + OMEGA * trial
                    })
                } else {
                    self.history[owner][k]
                };
                if update {
                    self.history[owner][k] = force;
                }
                markers.push(Marker {
                    owner,
                    selected_node: node,
                    weights,
                    force,
                });
            }
        }
        Ok((markers, active))
    }

    fn solve_markers(&mut self, markers: &[Marker]) -> Result<Vec<f64>, String> {
        let mut rhs = vec![0.0; self.grid.velocity_count()];
        for m in markers {
            for axis in 0..2 {
                for &(k, w) in &m.weights[axis] {
                    rhs[k] += m.force[axis] * w / (self.grid.h * self.grid.h);
                }
            }
        }
        let feedback = self.grid.solve(&rhs, &|_| 0.0)?;
        for (out, (base, delta)) in self
            .combined
            .velocity
            .iter_mut()
            .zip(self.background.velocity.iter().zip(&feedback.velocity))
        {
            *out = base * self.pressure + delta;
        }
        for (out, (base, delta)) in self
            .combined
            .pressure
            .iter_mut()
            .zip(self.background.pressure.iter().zip(&feedback.pressure))
        {
            *out = base * self.pressure + delta;
        }
        self.feedback = feedback;
        Ok(rhs)
    }

    fn prime_from_history(
        &mut self,
        world: &PhysicsWorld,
        cells: &[SoftBodyHandle],
        robot: RigidBodyHandle,
        drag: f32,
        radius: f32,
    ) -> Result<(), String> {
        let (markers, _) = self.collect_markers(world, cells, robot, drag, radius, false)?;
        self.solve_markers(&markers)?;
        Ok(())
    }

    pub fn advance(
        &mut self,
        world: &mut PhysicsWorld,
        cells: &[SoftBodyHandle],
        robot: RigidBodyHandle,
        dt: f32,
        drag: f32,
        radius: f32,
    ) -> Result<(), String> {
        let started = Instant::now();
        let (markers, active) = self.collect_markers(world, cells, robot, drag, radius, true)?;
        let rhs = self.solve_markers(&markers)?;
        let full_rhs: Vec<_> = rhs
            .iter()
            .zip(&self.background_rhs)
            .map(|(f, b)| f + self.pressure * b)
            .collect();
        let (divergence, momentum, flux) =
            self.grid
                .measure(&self.combined.velocity, &self.combined.pressure, &full_rhs);
        let mut body_impulse = [0.0; 2];
        for m in &markers {
            let impulse = Vector::new(
                (-m.force[0] * dt as f64) as f32,
                (-m.force[1] * dt as f64) as f32,
            );
            if let Some(node) = m.selected_node {
                let body = &mut world.soft_bodies[cells[m.owner]];
                let count = body.num_particles();
                let shared = impulse * (0.5 / count as f32);
                for j in 0..count {
                    body.apply_particle_impulse(j, shared, true);
                    body_impulse[0] += shared.x as f64;
                    body_impulse[1] += shared.y as f64;
                }
                let selected = impulse * 0.5;
                body.apply_particle_impulse(node, selected, true);
                body_impulse[0] += selected.x as f64;
                body_impulse[1] += selected.y as f64;
            } else {
                world.bodies[robot].apply_impulse(impulse, true);
                body_impulse[0] += impulse.x as f64;
                body_impulse[1] += impulse.y as f64;
            }
        }
        let split = self.grid.component_split();
        let fluid_impulse = [
            rhs[..split].iter().sum::<f64>() * self.grid.h * self.grid.h * dt as f64,
            rhs[split..].iter().sum::<f64>() * self.grid.h * self.grid.h * dt as f64,
        ];
        let error = (body_impulse[0] + fluid_impulse[0]).hypot(body_impulse[1] + fluid_impulse[1]);
        self.stats.active_cells = active;
        self.stats.solve_ms += started.elapsed().as_secs_f64() * 1000.0;
        self.stats.pressure_iterations += self.feedback.iterations;
        self.stats.divergence_rms = divergence;
        self.stats.momentum_rms = momentum;
        self.stats.net_flux = flux;
        self.stats.exchange_error = self.stats.exchange_error.max(error);
        Ok(())
    }
}

pub(crate) fn center(body: &SoftBody) -> Vector {
    body.particle_positions().sum::<Vector>() / body.num_particles() as f32
}

pub(crate) fn body_radius(body: &SoftBody) -> f32 {
    let middle = center(body);
    body.particle_positions()
        .map(|p| p.distance(middle))
        .fold(0.0, f32::max)
}
