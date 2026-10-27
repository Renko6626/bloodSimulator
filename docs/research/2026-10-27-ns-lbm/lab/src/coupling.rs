//! Existing permeable four-marker cell model, separate from fluid advancement.
use crate::grid::{Grid, weights};
use rapier2d::prelude::*;
const NODES: usize = 12;
const GAMMA: f64 = 0.25;

pub struct Cells {
    pub world: PhysicsWorld,
    handles: Vec<SoftBodyHandle>,
    areas: Vec<f64>,
}
fn area(b: &SoftBody) -> f64 {
    let p: Vec<_> = b.particle_positions().collect();
    (0..NODES)
        .map(|i| p[i].perp_dot(p[(i + 1) % NODES]) as f64)
        .sum::<f64>()
        * 0.5
}
impl Cells {
    pub fn new(g: &Grid, count: usize) -> Self {
        let mut world = PhysicsWorld::new();
        world.gravity = Vector::ZERO;
        world.integration_parameters.dt = g.c.dt as f32;
        for y in [-0.5, g.c.ly + 0.5] {
            world.insert(
                RigidBodyBuilder::fixed().translation(Vector::new((g.c.lx * 0.5) as f32, y as f32)),
                ColliderBuilder::cuboid((g.c.lx * 2.0) as f32, 0.5).friction(0.12),
            );
        }
        let columns = if g.c.ly < 10.0 { 24 } else { 12 };
        let rows = count.div_ceil(columns);
        let mut handles = Vec::new();
        for i in 0..count {
            let center = [(i % columns) as f64 + 0.5, (i / columns) as f64 + 0.5];
            let p = [
                center[0] * g.c.lx / columns as f64,
                center[1] * g.c.ly / rows as f64,
            ];
            let handle = world.insert_soft_body(
                SoftBodyBuilder::disk(Vector::new(p[0] as f32, p[1] as f32), 0.7, NODES)
                    .mass(1.0)
                    .material(SoftBodyMaterial {
                        volume_softness: SpringCoefficients::new(30.0, 1.0),
                        ..SoftBodyMaterial::uniform(SpringCoefficients::new(12.0, 1.0))
                    })
                    .linear_damping(0.5)
                    .particle_radius(0.07)
                    .surface_collider(ColliderBuilder::ball(0.07).friction(0.12))
                    .can_sleep(false),
            );
            handles.push(handle);
        }
        let areas = handles
            .iter()
            .map(|&h| area(&world.soft_bodies[h]))
            .collect();
        Self {
            world,
            handles,
            areas,
        }
    }
    /// Simultaneous explicit drag from the pre-exchange velocity. Return the
    /// norm of the requested body+actual grid impulse (includes f32 conversion).
    /// Points here stay off walls; an intersecting wall stencil is rejected.
    pub fn exchange(&mut self, g: &mut Grid) -> f64 {
        let mut exchanges = Vec::with_capacity(self.handles.len() * 4);
        for &h in &self.handles {
            let b = &self.world.soft_bodies[h];
            let center = b.particle_positions().sum::<Vector>() / NODES as f32;
            let mean = b.particle_velocities().sum::<Vector>() / NODES as f32;
            for k in 0..4 {
                let p = (b.particle_position(3 * k) + center) * 0.5;
                let v = (b.particle_velocity(3 * k) + mean) * 0.5;
                let pos = [p.x as f64, p.y as f64];
                let u = g.velocity(pos);
                let q = [
                    GAMMA / 4.0 * g.c.dt * (u[0] - v.x as f64),
                    GAMMA / 4.0 * g.c.dt * (u[1] - v.y as f64),
                ];
                exchanges.push((h, k, pos, q));
            }
        }
        let mut mismatch = [0.0; 2];
        let mass = g.c.hx() * g.c.hy();
        for (h, k, p, q) in exchanges {
            let body = &mut self.world.soft_bodies[h];
            for i in 0..NODES {
                let w = 0.5 / NODES as f64 + if i == 3 * k { 0.5 } else { 0.0 };
                let impulse = Vector::new((w * q[0]) as f32, (w * q[1]) as f32);
                mismatch[0] += impulse.x as f64;
                mismatch[1] += impulse.y as f64;
                body.apply_particle_impulse(i, impulse, true);
            }
            for (component, field) in [(0, &mut g.u), (1, &mut g.v)] {
                let ws = weights(g.c, component, p);
                assert!(
                    (ws.iter().map(|x| x.1).sum::<f64>() - 1.0).abs() < 1e-10,
                    "marker touches wall interpolation zone"
                );
                for (idx, w) in ws {
                    if let Some(idx) = idx {
                        let before = field[idx];
                        field[idx] -= w * q[component] / mass;
                        mismatch[component] += (field[idx] - before) * mass;
                    }
                }
            }
        }
        mismatch[0].hypot(mismatch[1])
    }
    pub fn step(&mut self, lx: f64) {
        self.world.step();
        // Whole-cell periodic wrap; cross-seam cell-cell contacts are omitted.
        for &h in &self.handles {
            let b = &mut self.world.soft_bodies[h];
            let x = b.particle_positions().map(|p| p.x as f64).sum::<f64>() / NODES as f64;
            let shift = x.rem_euclid(lx) - x;
            if shift.abs() > 0.5 * lx {
                for i in 0..NODES {
                    b.set_particle_position(
                        i,
                        b.particle_position(i) + Vector::new(shift as f32, 0.0),
                    );
                }
            }
        }
    }
    /// Maximum fractional area change, max node speed, mean cell axial speed.
    pub fn diagnostics(&self) -> (f64, f64, f64) {
        let mut a: f64 = 0.0;
        let mut speed: f64 = 0.0;
        let mut vx = 0.0;
        for (idx, &h) in self.handles.iter().enumerate() {
            let b = &self.world.soft_bodies[h];
            assert!(b.particle_positions().all(|p| p.is_finite()));
            a = a.max((area(b) / self.areas[idx] - 1.0).abs());
            for v in b.particle_velocities() {
                assert!(v.is_finite());
                speed = speed.max(v.length() as f64);
                vx += v.x as f64;
            }
        }
        (a, speed, vx / (self.handles.len() * NODES).max(1) as f64)
    }
    #[cfg(test)]
    pub fn momentum(&self) -> [f64; 2] {
        let mut p = [0.0; 2];
        for &h in &self.handles {
            for v in self.world.soft_bodies[h].particle_velocities() {
                p[0] += v.x as f64 / NODES as f64;
                p[1] += v.y as f64 / NODES as f64;
            }
        }
        p
    }
}
