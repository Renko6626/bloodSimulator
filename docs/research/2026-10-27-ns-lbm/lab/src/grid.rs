//! Rectangular MAC discretization. Density and 2D effective thickness are one.
use crate::factor::Factor;
use faer::sparse::Triplet;
use rayon::prelude::*;
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub nx: usize,
    pub ny: usize,
    pub lx: f64,
    pub ly: f64,
    pub dt: f64,
    pub nu: f64,
    pub periodic_y: bool,
}
impl Config {
    pub fn new(nx: usize, ny: usize, lx: f64, ly: f64, dt: f64, nu: f64, periodic_y: bool) -> Self {
        assert!(nx >= 3 && ny >= 3 && lx > 0.0 && ly > 0.0 && dt > 0.0 && nu >= 0.0);
        Self {
            nx,
            ny,
            lx,
            ly,
            dt,
            nu,
            periodic_y,
        }
    }
    pub fn hx(self) -> f64 {
        self.lx / self.nx as f64
    }
    pub fn hy(self) -> f64 {
        self.ly / self.ny as f64
    }
    pub fn rows(self, component: usize) -> usize {
        if component == 1 && !self.periodic_y {
            self.ny - 1
        } else {
            self.ny
        }
    }
    pub fn position(self, component: usize, index: usize) -> [f64; 2] {
        let i = index % self.nx;
        let j = index / self.nx;
        if component == 0 {
            [i as f64 * self.hx(), (j as f64 + 0.5) * self.hy()]
        } else {
            [
                (i as f64 + 0.5) * self.hx(),
                (j as f64 + if self.periodic_y { 0.0 } else { 1.0 }) * self.hy(),
            ]
        }
    }
}
#[derive(Default, Clone, Copy)]
pub struct Times {
    pub advection: f64,
    pub diffusion: f64,
    pub pressure: f64,
}
impl std::ops::AddAssign for Times {
    fn add_assign(&mut self, x: Self) {
        self.advection += x.advection;
        self.diffusion += x.diffusion;
        self.pressure += x.pressure;
    }
}

/// Four native-grid interpolation weights. Odd wall reflection supplies no-slip
/// ghosts. A None index is a prescribed-zero wall face (normal component).
pub fn weights(c: Config, component: usize, p: [f64; 2]) -> [(Option<usize>, f64); 4] {
    let x = p[0] / c.hx() - if component == 0 { 0.0 } else { 0.5 };
    let y = p[1] / c.hy()
        - if component == 0 {
            0.5
        } else if c.periodic_y {
            0.0
        } else {
            1.0
        };
    let i = x.floor() as isize;
    let j = y.floor() as isize;
    let tx = x - x.floor();
    let ty = y - y.floor();
    std::array::from_fn(|k| {
        let di = k % 2;
        let dj = k / 2;
        let ix = (i + di as isize).rem_euclid(c.nx as isize) as usize;
        let raw = j + dj as isize;
        let w = if di == 0 { 1.0 - tx } else { tx } * if dj == 0 { 1.0 - ty } else { ty };
        let (iy, sign) = if c.periodic_y {
            (Some(raw.rem_euclid(c.ny as isize) as usize), 1.0)
        } else if component == 0 {
            let m = raw.rem_euclid(2 * c.ny as isize) as usize;
            if m < c.ny {
                (Some(m), 1.0)
            } else {
                (Some(2 * c.ny - 1 - m), -1.0)
            }
        } else {
            let m = (raw + 1).rem_euclid(2 * c.ny as isize) as usize;
            if m == 0 || m == c.ny {
                (None, 0.0)
            } else if m < c.ny {
                (Some(m - 1), 1.0)
            } else {
                (Some(2 * c.ny - m - 1), -1.0)
            }
        };
        (iy.map(|y| y * c.nx + ix), w * sign)
    })
}
fn sample(c: Config, field: &[f64], component: usize, p: [f64; 2]) -> f64 {
    weights(c, component, p)
        .iter()
        .map(|&(i, w)| i.map_or(0.0, |i| field[i] * w))
        .sum()
}
fn vector_sample(c: Config, u: &[f64], v: &[f64], p: [f64; 2]) -> [f64; 2] {
    [sample(c, u, 0, p), sample(c, v, 1, p)]
}
fn add(entries: &mut Vec<Triplet<usize, usize, f64>>, i: usize, j: usize, v: f64) {
    entries.push(Triplet::new(i, j, v));
}
fn helmholtz(c: Config, component: usize) -> Factor {
    let rows = c.rows(component);
    let n = c.nx * rows;
    let ax = c.dt * c.nu / c.hx().powi(2);
    let ay = c.dt * c.nu / c.hy().powi(2);
    let mut e = Vec::with_capacity(5 * n);
    for j in 0..rows {
        for i in 0..c.nx {
            let k = j * c.nx + i;
            let mut diag = 1.0 + 2.0 * ax;
            add(&mut e, k, j * c.nx + (i + 1) % c.nx, -ax);
            add(&mut e, k, j * c.nx + (i + c.nx - 1) % c.nx, -ax);
            for dj in [-1, 1] {
                let jj = j as isize + dj;
                if c.periodic_y {
                    diag += ay;
                    add(
                        &mut e,
                        k,
                        jj.rem_euclid(rows as isize) as usize * c.nx + i,
                        -ay,
                    );
                } else if jj >= 0 && jj < rows as isize {
                    diag += ay;
                    add(&mut e, k, jj as usize * c.nx + i, -ay);
                } else {
                    diag += if component == 0 { 2.0 * ay } else { ay };
                }
            }
            add(&mut e, k, k, diag);
        }
    }
    Factor::new(n, &e)
}
fn pressure(c: Config) -> Factor {
    let mut e = Vec::with_capacity(5 * c.nx * c.ny);
    let ax = 1.0 / c.hx().powi(2);
    let ay = 1.0 / c.hy().powi(2);
    for j in 0..c.ny {
        for i in 0..c.nx {
            let k = j * c.nx + i;
            if k == 0 {
                continue;
            }
            let mut diag = 2.0 * ax;
            for ii in [(i + 1) % c.nx, (i + c.nx - 1) % c.nx] {
                let q = j * c.nx + ii;
                if q != 0 {
                    add(&mut e, k - 1, q - 1, -ax);
                }
            }
            for dj in [-1, 1] {
                let jj = j as isize + dj;
                if c.periodic_y || (jj >= 0 && jj < c.ny as isize) {
                    diag += ay;
                    let q = jj.rem_euclid(c.ny as isize) as usize * c.nx + i;
                    if q != 0 {
                        add(&mut e, k - 1, q - 1, -ay);
                    }
                }
            }
            add(&mut e, k - 1, k - 1, diag);
        }
    }
    Factor::new(c.nx * c.ny - 1, &e)
}

pub struct Grid {
    pub c: Config,
    pub u: Vec<f64>,
    pub v: Vec<f64>,
    next_u: Vec<f64>,
    next_v: Vec<f64>,
    p: Vec<f64>,
    diffusion_u: Factor,
    diffusion_v: Factor,
    projection: Factor,
}
impl Grid {
    pub fn new(c: Config) -> Self {
        let n = c.nx * c.ny;
        let nv = c.nx * c.rows(1);
        Self {
            c,
            u: vec![0.0; n],
            v: vec![0.0; nv],
            next_u: vec![0.0; n],
            next_v: vec![0.0; nv],
            p: vec![0.0; n - 1],
            diffusion_u: helmholtz(c, 0),
            diffusion_v: helmholtz(c, 1),
            projection: pressure(c),
        }
    }
    pub fn fingerprint(&self) -> String {
        let mut h = 0xcbf29ce484222325u64;
        for x in self.u.iter().chain(&self.v) {
            for b in x.to_bits().to_le_bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100000001b3);
            }
        }
        format!("{h:016x}")
    }
    pub fn factor_bytes(&self) -> usize {
        self.diffusion_u.numeric_bytes()
            + self.diffusion_v.numeric_bytes()
            + self.projection.numeric_bytes()
    }
    pub fn set_velocity(&mut self, f: impl Fn(f64, f64) -> [f64; 2]) {
        for (component, field) in [(0, &mut self.u), (1, &mut self.v)] {
            for (k, val) in field.iter_mut().enumerate() {
                let [x, y] = self.c.position(component, k);
                *val = f(x, y)[component];
            }
        }
    }
    pub fn velocity(&self, p: [f64; 2]) -> [f64; 2] {
        vector_sample(self.c, &self.u, &self.v, p)
    }
    pub fn advect(&mut self) {
        let _span = crate::profile::span("advection", 1);
        let c = self.c;
        let u = &self.u;
        let v = &self.v;
        for (component, out) in [(0, &mut self.next_u), (1, &mut self.next_v)] {
            out.par_iter_mut().enumerate().for_each(|(k, val)| {
                let p = c.position(component, k);
                let a = vector_sample(c, u, v, p);
                let mid = [p[0] - 0.5 * c.dt * a[0], p[1] - 0.5 * c.dt * a[1]];
                let b = vector_sample(c, u, v, mid);
                let back = [p[0] - c.dt * b[0], p[1] - c.dt * b[1]];
                *val = sample(c, if component == 0 { u } else { v }, component, back);
            });
        }
        std::mem::swap(&mut self.u, &mut self.next_u);
        std::mem::swap(&mut self.v, &mut self.next_v);
    }
    pub fn diffuse(&mut self) {
        {
            let _span = crate::profile::span("diffusion_u", 3);
            self.diffusion_u.solve(&mut self.u);
        }
        {
            let _span = crate::profile::span("diffusion_v", 4);
            self.diffusion_v.solve(&mut self.v);
        }
    }
    fn divergence_at(&self, i: usize, j: usize) -> f64 {
        let c = self.c;
        let right = self.u[j * c.nx + (i + 1) % c.nx];
        let left = self.u[j * c.nx + i];
        let (bottom, top) = if c.periodic_y {
            (self.v[j * c.nx + i], self.v[((j + 1) % c.ny) * c.nx + i])
        } else {
            (
                if j == 0 {
                    0.0
                } else {
                    self.v[(j - 1) * c.nx + i]
                },
                if j == c.ny - 1 {
                    0.0
                } else {
                    self.v[j * c.nx + i]
                },
            )
        };
        (right - left) / c.hx() + (top - bottom) / c.hy()
    }
    pub fn project(&mut self) {
        let c = self.c;
        let rhs_span = crate::profile::span("pressure_rhs", 5);
        for k in 1..c.nx * c.ny {
            self.p[k - 1] = -self.divergence_at(k % c.nx, k / c.nx) / c.dt;
        }
        drop(rhs_span);
        {
            let _span = crate::profile::span("pressure_solve", 6);
            self.projection.solve(&mut self.p);
        }
        let _span = crate::profile::span("pressure_correct", 7);
        let p = |k: usize| if k == 0 { 0.0 } else { self.p[k - 1] };
        for j in 0..c.ny {
            for i in 0..c.nx {
                self.u[j * c.nx + i] +=
                    c.dt / c.hx() * (p(j * c.nx + (i + c.nx - 1) % c.nx) - p(j * c.nx + i));
            }
        }
        for j in 0..c.rows(1) {
            for i in 0..c.nx {
                let (minus, plus) = if c.periodic_y {
                    ((j + c.ny - 1) % c.ny, j)
                } else {
                    (j, j + 1)
                };
                self.v[j * c.nx + i] += c.dt / c.hy() * (p(minus * c.nx + i) - p(plus * c.nx + i));
            }
        }
    }
    pub fn step(&mut self, force_x: f64) -> Times {
        let t = Instant::now();
        self.advect();
        let advection = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        {
            let _span = crate::profile::span("drive", 2);
            for x in &mut self.u {
                *x += self.c.dt * force_x;
            }
        }
        self.diffuse();
        let diffusion = t.elapsed().as_secs_f64() * 1e3;
        let t = Instant::now();
        self.project();
        let pressure = t.elapsed().as_secs_f64() * 1e3;
        Times {
            advection,
            diffusion,
            pressure,
        }
    }
    /// RMS divergence, kinetic energy, maximum native component, mean axial flow.
    pub fn diagnostics(&self) -> (f64, f64, f64, f64) {
        let c = self.c;
        let mut d = 0.0;
        for j in 0..c.ny {
            for i in 0..c.nx {
                d += self.divergence_at(i, j).powi(2);
            }
        }
        let energy =
            0.5 * c.hx() * c.hy() * self.u.iter().chain(&self.v).map(|x| x * x).sum::<f64>();
        let max = self
            .u
            .iter()
            .chain(&self.v)
            .map(|x| x.abs())
            .fold(0.0, f64::max);
        let mean = self.u.iter().sum::<f64>() / (c.nx * c.ny) as f64;
        ((d / (c.nx * c.ny) as f64).sqrt(), energy, max, mean)
    }
    pub fn relative_error(&self, f: impl Fn(f64, f64) -> [f64; 2]) -> f64 {
        let mut error = 0.0;
        let mut norm = 0.0;
        for (component, field) in [(0, &self.u), (1, &self.v)] {
            for (k, &val) in field.iter().enumerate() {
                let [x, y] = self.c.position(component, k);
                let exact = f(x, y)[component];
                error += (val - exact).powi(2);
                norm += exact.powi(2);
            }
        }
        (error / norm.max(1e-30)).sqrt()
    }
}
