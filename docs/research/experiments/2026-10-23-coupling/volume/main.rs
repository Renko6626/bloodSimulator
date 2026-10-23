use std::env;
use std::f64::consts::PI;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

mod separable;
use separable::SeparableInverse;

struct Grid {
    n: usize,
    h: f64,
    len: usize,
}
impl Grid {
    fn new(n: usize) -> Self {
        Self {
            n,
            h: 1.0 / n as f64,
            len: n * n + n * (n - 1),
        }
    }
    fn u(&self, i: usize, j: usize) -> usize {
        j * self.n + i % self.n
    }
    fn v(&self, i: usize, j: usize) -> usize {
        self.n * self.n + (j - 1) * self.n + i % self.n
    }
    fn p(&self, i: usize, j: usize) -> usize {
        j * self.n + i % self.n
    }
    fn apply_a(&self, x: &[f64], out: &mut [f64]) {
        let n = self.n;
        let ih2 = 1.0 / (self.h * self.h);
        for j in 0..n {
            for i in 0..n {
                let c = x[self.u(i, j)];
                let left = x[self.u((i + n - 1) % n, j)];
                let right = x[self.u((i + 1) % n, j)];
                let below = if j == 0 { -c } else { x[self.u(i, j - 1)] };
                let above = if j + 1 == n { -c } else { x[self.u(i, j + 1)] };
                out[self.u(i, j)] = (4.0 * c - left - right - below - above) * ih2;
            }
        }
        for j in 1..n {
            for i in 0..n {
                let c = x[self.v(i, j)];
                let left = x[self.v((i + n - 1) % n, j)];
                let right = x[self.v((i + 1) % n, j)];
                let below = if j == 1 { 0.0 } else { x[self.v(i, j - 1)] };
                let above = if j + 1 == n { 0.0 } else { x[self.v(i, j + 1)] };
                out[self.v(i, j)] = (4.0 * c - left - right - below - above) * ih2;
            }
        }
    }
    fn gradient(&self, p: &[f64], out: &mut [f64]) {
        let n = self.n;
        for j in 0..n {
            for i in 0..n {
                out[self.u(i, j)] = (p[self.p(i, j)] - p[self.p((i + n - 1) % n, j)]) / self.h;
            }
        }
        for j in 1..n {
            for i in 0..n {
                out[self.v(i, j)] = (p[self.p(i, j)] - p[self.p(i, j - 1)]) / self.h;
            }
        }
    }
    fn divergence(&self, u: &[f64], out: &mut [f64]) {
        let n = self.n;
        for j in 0..n {
            for i in 0..n {
                let bottom = if j == 0 { 0.0 } else { u[self.v(i, j)] };
                let top = if j + 1 == n { 0.0 } else { u[self.v(i, j + 1)] };
                out[self.p(i, j)] =
                    (u[self.u((i + 1) % n, j)] - u[self.u(i, j)] + top - bottom) / self.h;
            }
        }
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}
fn zero_mean(a: &mut [f64]) {
    let m = a.iter().sum::<f64>() / a.len() as f64;
    for x in a {
        *x -= m;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Geometry {
    Channel,
    DiskStatic,
    DiskMoving,
}

impl Geometry {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "channel" => Ok(Self::Channel),
            "disk-static" => Ok(Self::DiskStatic),
            "disk-moving" => Ok(Self::DiskMoving),
            _ => Err(format!("unknown scene {s}")),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Channel => "channel",
            Self::DiskStatic => "disk-static",
            Self::DiskMoving => "disk-moving",
        }
    }
    fn wall(x: f64) -> (f64, f64, f64, f64) {
        let a = 2.0 * PI * x;
        // Smooth periodic contraction; lower and upper walls never meet.
        (
            0.12 + 0.04 * a.sin(),
            0.88 - 0.08 * a.sin(),
            0.08 * PI * a.cos(),
            -0.16 * PI * a.cos(),
        )
    }
    fn solid(self, x: f64, y: f64) -> bool {
        match self {
            Self::Channel => {
                let (lo, hi, _, _) = Self::wall(x);
                y < lo || y > hi
            }
            _ => (x - 0.5).powi(2) + (y - 0.5).powi(2) < 0.15f64.powi(2),
        }
    }
    fn prescribed(self) -> (f64, f64) {
        if self == Self::DiskMoving {
            (0.1, 0.0)
        } else {
            (0.0, 0.0)
        }
    }
    fn forcing(self) -> (f64, f64) {
        if self == Self::DiskMoving {
            (0.0, 0.0)
        } else {
            (1.0, 0.0)
        }
    }
}

struct Brinkman {
    scene: Geometry,
    kappa: f64,
    mask: Vec<f64>,
    target: Vec<f64>,
    rhs: Vec<f64>,
}
impl Brinkman {
    fn new(g: &Grid, scene: Geometry, kappa: f64) -> Self {
        let mut mask = vec![0.0; g.len];
        let mut target = vec![0.0; g.len];
        let mut rhs = vec![0.0; g.len];
        let (vx, vy) = scene.prescribed();
        let (fx, fy) = scene.forcing();
        for j in 0..g.n {
            for i in 0..g.n {
                let x = i as f64 * g.h;
                let y = (j as f64 + 0.5) * g.h;
                let q = g.u(i, j);
                mask[q] = scene.solid(x, y) as u8 as f64;
                target[q] = vx;
                rhs[q] = fx + kappa * mask[q] * vx;
            }
        }
        for j in 1..g.n {
            for i in 0..g.n {
                let x = (i as f64 + 0.5) * g.h;
                let y = j as f64 * g.h;
                let q = g.v(i, j);
                mask[q] = scene.solid(x, y) as u8 as f64;
                target[q] = vy;
                rhs[q] = fy + kappa * mask[q] * vy;
            }
        }
        Self {
            scene,
            kappa,
            mask,
            target,
            rhs,
        }
    }
    fn apply(&self, g: &Grid, x: &[f64], out: &mut [f64]) {
        g.apply_a(x, out);
        for i in 0..g.len {
            out[i] += self.kappa * self.mask[i] * x[i];
        }
    }
}

#[derive(Default)]
struct Counters {
    velocity_calls: usize,
    velocity_iters: usize,
    pressure_iters: usize,
}

fn solve_velocity(
    g: &Grid,
    b: &Brinkman,
    pre: &SeparableInverse,
    rhs: &[f64],
    x: &mut [f64],
    c: &mut Counters,
) -> Result<(), String> {
    c.velocity_calls += 1;
    let mut bx = vec![0.0; g.len];
    b.apply(g, x, &mut bx);
    let mut r: Vec<f64> = rhs.iter().zip(&bx).map(|(a, v)| a - v).collect();
    let tol = 5e-12 * norm(rhs).max(1.0);
    if norm(&r) <= tol {
        return Ok(());
    }
    let mut z = vec![0.0; g.len];
    pre.solve(&r, &mut z);
    let mut d = z.clone();
    let mut rz = dot(&r, &z);
    for _ in 0..(20 * g.n) {
        b.apply(g, &d, &mut bx);
        let denom = dot(&d, &bx);
        if denom <= 0.0 {
            return Err("velocity PCG lost SPD".into());
        }
        let alpha = rz / denom;
        for i in 0..g.len {
            x[i] += alpha * d[i];
            r[i] -= alpha * bx[i];
        }
        c.velocity_iters += 1;
        if norm(&r) <= tol {
            // Check true B residual because Schur CG uses this approximate inverse.
            b.apply(g, x, &mut bx);
            for i in 0..g.len {
                r[i] = rhs[i] - bx[i];
            }
            if norm(&r) <= tol {
                return Ok(());
            }
        }
        pre.solve(&r, &mut z);
        let next = dot(&r, &z);
        let beta = next / rz;
        for i in 0..g.len {
            d[i] = z[i] + beta * d[i];
        }
        rz = next;
    }
    Err(format!(
        "velocity PCG failed, residual={:.3e}, tol={tol:.3e}",
        norm(&r)
    ))
}

fn apply_s(
    g: &Grid,
    b: &Brinkman,
    pre: &SeparableInverse,
    p: &[f64],
    out: &mut [f64],
    c: &mut Counters,
) -> Result<(), String> {
    let mut grad = vec![0.0; g.len];
    let mut z = vec![0.0; g.len];
    g.gradient(p, &mut grad);
    solve_velocity(g, b, pre, &grad, &mut z, c)?;
    g.divergence(&z, out);
    for v in out.iter_mut() {
        *v = -*v;
    }
    zero_mean(out);
    Ok(())
}

fn solve(
    g: &Grid,
    b: &Brinkman,
    pre: &SeparableInverse,
) -> Result<(Vec<f64>, Vec<f64>, Counters), String> {
    let mut c = Counters::default();
    let mut binvf = vec![0.0; g.len];
    solve_velocity(g, b, pre, &b.rhs, &mut binvf, &mut c)?;
    let mut rhs = vec![0.0; g.n * g.n];
    g.divergence(&binvf, &mut rhs);
    for v in rhs.iter_mut() {
        *v = -*v;
    }
    zero_mean(&mut rhs);
    let mut p = vec![0.0; rhs.len()];
    let mut r = rhs.clone();
    let mut d = r.clone();
    let mut rr = dot(&r, &r);
    let tol = 2e-9 * norm(&rhs).max(1.0);
    let mut sd = vec![0.0; rhs.len()];
    while rr.sqrt() > tol {
        if c.pressure_iters >= 12 * g.n {
            return Err(format!(
                "Schur CG failed, residual={:.3e}, tol={tol:.3e}",
                rr.sqrt()
            ));
        }
        apply_s(g, b, pre, &d, &mut sd, &mut c)?;
        let denom = dot(&d, &sd);
        if denom <= 0.0 {
            return Err(format!("Schur CG lost SPD: {denom:.3e}"));
        }
        let alpha = rr / denom;
        for i in 0..p.len() {
            p[i] += alpha * d[i];
            r[i] -= alpha * sd[i];
        }
        zero_mean(&mut r);
        c.pressure_iters += 1;
        let next = dot(&r, &r);
        if next.sqrt() <= tol {
            break;
        }
        let beta = next / rr;
        for i in 0..d.len() {
            d[i] = r[i] + beta * d[i];
        }
        zero_mean(&mut d);
        rr = next;
    }
    zero_mean(&mut p);
    let mut gp = vec![0.0; g.len];
    g.gradient(&p, &mut gp);
    let final_rhs: Vec<f64> = b.rhs.iter().zip(&gp).map(|(f, v)| f - v).collect();
    let mut u = vec![0.0; g.len];
    solve_velocity(g, b, pre, &final_rhs, &mut u, &mut c)?;
    Ok((u, p, c))
}

fn velocity(g: &Grid, u: &[f64], x: f64, y: f64) -> (f64, f64) {
    fn interp(g: &Grid, u: &[f64], x: f64, y: f64, ux: bool) -> f64 {
        let xx = if ux { x / g.h } else { x / g.h - 0.5 };
        let yy = if ux { y / g.h - 0.5 } else { y / g.h };
        let i0 = xx.floor() as isize;
        let j0 = yy.floor() as isize;
        let tx = xx - i0 as f64;
        let ty = yy - j0 as f64;
        let get = |i: isize, j: isize| -> f64 {
            let ii = i.rem_euclid(g.n as isize) as usize;
            if ux {
                if j < 0 {
                    -u[g.u(ii, 0)]
                } else if j >= g.n as isize {
                    -u[g.u(ii, g.n - 1)]
                } else {
                    u[g.u(ii, j as usize)]
                }
            } else if j <= 0 || j >= g.n as isize {
                0.0
            } else {
                u[g.v(ii, j as usize)]
            }
        };
        (1.0 - tx) * ((1.0 - ty) * get(i0, j0) + ty * get(i0, j0 + 1))
            + tx * ((1.0 - ty) * get(i0 + 1, j0) + ty * get(i0 + 1, j0 + 1))
    }
    (interp(g, u, x, y, true), interp(g, u, x, y, false))
}

#[derive(Default)]
struct Metrics {
    mom_rms: f64,
    div_rms: f64,
    tang_rms: f64,
    normal_rms: f64,
    abs_normal_flux: f64,
    full_q_min: f64,
    full_q_max: f64,
    fluid_q_min: f64,
    fluid_q_max: f64,
    rx: f64,
    ry: f64,
    torque: f64,
}
fn metrics(g: &Grid, b: &Brinkman, u: &[f64], p: &[f64]) -> Metrics {
    let mut m = Metrics::default();
    let mut bu = vec![0.0; g.len];
    let mut gp = vec![0.0; g.len];
    b.apply(g, u, &mut bu);
    g.gradient(p, &mut gp);
    m.mom_rms = (bu
        .iter()
        .zip(&gp)
        .zip(&b.rhs)
        .map(|((a, v), f)| (a + v - f).powi(2))
        .sum::<f64>()
        / g.len as f64)
        .sqrt();
    let mut div = vec![0.0; g.n * g.n];
    g.divergence(u, &mut div);
    m.div_rms = (dot(&div, &div) / div.len() as f64).sqrt();
    for q in 0..g.len {
        let (x, y, ux) = if q < g.n * g.n {
            let j = q / g.n;
            let i = q % g.n;
            (i as f64 * g.h, (j as f64 + 0.5) * g.h, true)
        } else {
            let k = q - g.n * g.n;
            let j = k / g.n + 1;
            let i = k % g.n;
            ((i as f64 + 0.5) * g.h, j as f64 * g.h, false)
        };
        let reaction = b.kappa * b.mask[q] * (u[q] - b.target[q]) * g.h * g.h;
        if ux {
            m.rx += reaction;
            m.torque -= (y - 0.5) * reaction;
        } else {
            m.ry += reaction;
            m.torque += (x - 0.5) * reaction;
        }
    }
    m.full_q_min = f64::INFINITY;
    m.fluid_q_min = f64::INFINITY;
    m.full_q_max = f64::NEG_INFINITY;
    m.fluid_q_max = f64::NEG_INFINITY;
    for i in 0..g.n {
        let mut full = 0.0;
        let mut fluid = 0.0;
        for j in 0..g.n {
            let q = g.u(i, j);
            full += u[q] * g.h;
            fluid += (1.0 - b.mask[q]) * u[q] * g.h;
        }
        m.full_q_min = m.full_q_min.min(full);
        m.full_q_max = m.full_q_max.max(full);
        m.fluid_q_min = m.fluid_q_min.min(fluid);
        m.fluid_q_max = m.fluid_q_max.max(fluid);
    }
    let samples = 256;
    let mut st = 0.0;
    let mut sn = 0.0;
    let mut length = 0.0;
    let (vx, vy) = b.scene.prescribed();
    let curves = if b.scene == Geometry::Channel { 2 } else { 1 };
    for curve in 0..curves {
        for k in 0..samples {
            let (x, y, tx, ty, ds) = if b.scene == Geometry::Channel {
                let x = (k as f64 + 0.5) / samples as f64;
                let (lo, hi, dlo, dhi) = Geometry::wall(x);
                let (y, slope) = if curve == 0 { (lo, dlo) } else { (hi, dhi) };
                let scale = (1.0 + slope * slope).sqrt();
                (x, y, 1.0 / scale, slope / scale, scale / samples as f64)
            } else {
                let theta = 2.0 * PI * (k as f64 + 0.5) / samples as f64;
                (
                    0.5 + 0.15 * theta.cos(),
                    0.5 + 0.15 * theta.sin(),
                    -theta.sin(),
                    theta.cos(),
                    2.0 * PI * 0.15 / samples as f64,
                )
            };
            let (ux, uy) = velocity(g, u, x, y);
            let dx = ux - vx;
            let dy = uy - vy;
            let t = dx * tx + dy * ty;
            let n = -dx * ty + dy * tx;
            st += t * t * ds;
            sn += n * n * ds;
            m.abs_normal_flux += n.abs() * ds;
            length += ds;
        }
    }
    m.tang_rms = (st / length).sqrt();
    m.normal_rms = (sn / length).sqrt();
    m
}

fn write_field(path: &str, g: &Grid, b: &Brinkman, u: &[f64], p: &[f64]) -> Result<(), String> {
    let f = File::create(path).map_err(|e| e.to_string())?;
    let mut w = BufWriter::new(f);
    writeln!(w, "x,y,u,v,p,solid").map_err(|e| e.to_string())?;
    for j in 0..g.n {
        for i in 0..g.n {
            let x = (i as f64 + 0.5) * g.h;
            let y = (j as f64 + 0.5) * g.h;
            let (vx, vy) = velocity(g, u, x, y);
            writeln!(
                w,
                "{x:.8},{y:.8},{vx:.12e},{vy:.12e},{:.12e},{}",
                p[g.p(i, j)],
                b.scene.solid(x, y) as u8
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let mut n = 32usize;
    let mut scene = Geometry::Channel;
    let mut ell_h = Some(1.0);
    let mut kappa = None;
    let mut field = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {arg}"))?;
        match arg.as_str() {
            "--n" => n = value.parse().map_err(|_| "invalid n")?,
            "--scene" => scene = Geometry::parse(&value)?,
            "--ell-h" => {
                ell_h = Some(value.parse().map_err(|_| "invalid ell-h")?);
                kappa = None;
            }
            "--kappa" => {
                kappa = Some(value.parse().map_err(|_| "invalid kappa")?);
                ell_h = None;
            }
            "--field" => field = Some(value),
            _ => return Err(format!("unknown option {arg}")),
        }
    }
    if n < 8 || !n.is_power_of_two() {
        return Err("n must be power of two >=8".into());
    }
    let g = Grid::new(n);
    let kap = kappa.unwrap_or_else(|| 1.0 / (ell_h.unwrap() * g.h).powi(2));
    if !kap.is_finite() || kap <= 0.0 {
        return Err("kappa must be finite and positive".into());
    }
    let b = Brinkman::new(&g, scene, kap);
    let pre = SeparableInverse::new(n)?;
    let t = Instant::now();
    let (u, p, c) = solve(&g, &b, &pre)?;
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let m = metrics(&g, &b, &u, &p);
    if let Some(path) = field {
        write_field(&path, &g, &b, &u, &p)?;
    }
    println!("scene,n,kappa,ell_over_h,solve_ms,pressure_iters,velocity_calls,velocity_iters,mom_rms,div_rms,tang_rms,normal_rms,abs_normal_flux,full_q_min,full_q_max,fluid_q_min,fluid_q_max,reaction_x,reaction_y,torque");
    println!("{},{},{:.9e},{:.6},{:.6},{},{},{},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e}",scene.name(),n,kap,1.0/(kap.sqrt()*g.h),ms,c.pressure_iters,c.velocity_calls,c.velocity_iters,m.mom_rms,m.div_rms,m.tang_rms,m.normal_rms,m.abs_normal_flux,m.full_q_min,m.full_q_max,m.fluid_q_min,m.fluid_q_max,m.rx,m.ry,m.torque);
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
