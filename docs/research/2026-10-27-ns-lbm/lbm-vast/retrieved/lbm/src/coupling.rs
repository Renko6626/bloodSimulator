//! Existing permeable four-marker cell model, separate from fluid advancement.
use crate::model::{Config, Field};
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
    pub fn new(c: Config, count: usize) -> Self {
        let mut world = PhysicsWorld::new();
        world.gravity = Vector::ZERO;
        world.integration_parameters.dt = (1.0 / 120.0) as f32;
        for y in [-0.5, c.ly + 0.5] {
            world.insert(
                RigidBodyBuilder::fixed().translation(Vector::new((c.lx * 0.5) as f32, y as f32)),
                ColliderBuilder::cuboid((c.lx * 2.0) as f32, 0.5).friction(0.12),
            );
        }
        let columns = if c.ly < 10.0 { 24 } else { 12 };
        let rows = count.div_ceil(columns);
        let mut handles = Vec::new();
        for i in 0..count {
            let center = [(i % columns) as f64 + 0.5, (i / columns) as f64 + 0.5];
            let p = [
                center[0] * c.lx / columns as f64,
                center[1] * c.ly / rows as f64,
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
    /// Once per mechanical substep, matching NS exchange frequency. The
    /// collocated interpolation differs from the NS staggered grid weights.
    // Mismatch reports body requests plus uploaded f32 kicks, not a GPU readback audit.
    pub fn exchange(&mut self, c: Config, field: &Field) -> (Vec<[f32; 2]>, f64) {
        let mut kicks = vec![[0_f64; 2]; c.len()];
        let mut body_total = [0.; 2];
        for &h in &self.handles {
            let b = &mut self.world.soft_bodies[h];
            let center = b.particle_positions().sum::<Vector>() / NODES as f32;
            let mean = b.particle_velocities().sum::<Vector>() / NODES as f32;
            // Freeze all markers before applying any body's impulses.
            let markers: Vec<_> = (0..4)
                .map(|k| {
                    (
                        (b.particle_position(3 * k) + center) * 0.5,
                        (b.particle_velocity(3 * k) + mean) * 0.5,
                    )
                })
                .collect();
            for (k, (p, v)) in markers.into_iter().enumerate() {
                let ws = weights(c, [p.x as f64, p.y as f64]);
                let mut u = [0.; 2];
                for &(idx, w) in &ws {
                    for d in 0..2 {
                        u[d] += w * field.samples[idx][d + 1];
                    }
                }
                let q = [
                    GAMMA / 4. / 120. * (u[0] - v.x as f64),
                    GAMMA / 4. / 120. * (u[1] - v.y as f64),
                ];
                for i in 0..NODES {
                    let w = 0.5 / NODES as f64 + if i == 3 * k { 0.5 } else { 0. };
                    let impulse = Vector::new((w * q[0]) as f32, (w * q[1]) as f32);
                    b.apply_particle_impulse(i, impulse, true);
                    body_total[0] += impulse.x as f64;
                    body_total[1] += impulse.y as f64;
                }
                for &(idx, w) in &ws {
                    for d in 0..2 {
                        kicks[idx][d] -= w * q[d] / (c.h * c.h) * c.dt / c.h;
                    }
                }
            }
        }
        let kicks: Vec<[f32; 2]> = kicks
            .into_iter()
            .map(|v| [v[0] as f32, v[1] as f32])
            .collect();
        for k in &kicks {
            for d in 0..2 {
                body_total[d] += k[d] as f64 * c.h * c.h * c.h / c.dt;
            }
        }
        (kicks, body_total[0].hypot(body_total[1]))
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

fn weights(c: Config, p: [f64; 2]) -> [(usize, f64); 4] {
    let x = p[0] / c.h - 0.5;
    let y = p[1] / c.h - 0.5;
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f64;
    let fy = y - iy as f64;
    assert!(
        iy >= 0 && iy + 1 < c.ny as i32,
        "marker touches wall interpolation zone"
    );
    let idx =
        |dx: i32, dy: i32| (iy + dy) as usize * c.nx + (ix + dx).rem_euclid(c.nx as i32) as usize;
    [
        (idx(0, 0), (1. - fx) * (1. - fy)),
        (idx(1, 0), fx * (1. - fy)),
        (idx(0, 1), (1. - fx) * fy),
        (idx(1, 1), fx * fy),
    ]
}
