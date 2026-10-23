use std::env;
use std::f64::consts::PI;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
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

enum Backend {
    Cg,
    Separable(SeparableInverse),
}
impl Backend {
    fn new(name: &str, n: usize) -> Result<Self, String> {
        match name {
            "cg" => Ok(Self::Cg),
            "separable" => Ok(Self::Separable(SeparableInverse::new(n)?)),
            _ => Err(format!("invalid backend: {name}")),
        }
    }
}

struct Counters {
    inner_iters: usize,
    outer_iters: usize,
    inverse_calls: usize,
}

fn solve_velocity(
    g: &Grid,
    backend: &Backend,
    b: &[f64],
    x: &mut [f64],
    c: &mut Counters,
) -> Result<(), String> {
    c.inverse_calls += 1;
    match backend {
        Backend::Cg => solve_a(g, b, x, c),
        Backend::Separable(inv) => {
            inv.solve(b, x);
            Ok(())
        }
    }
}

fn solve_a(g: &Grid, b: &[f64], x: &mut [f64], c: &mut Counters) -> Result<(), String> {
    let mut ax = vec![0.0; g.len];
    g.apply_a(x, &mut ax);
    let mut r: Vec<f64> = b.iter().zip(&ax).map(|(bi, ai)| bi - ai).collect();
    let target = 2e-12 * norm(b).max(1.0);
    if norm(&r) <= target {
        return Ok(());
    }
    let mut d = r.clone();
    let mut rr = dot(&r, &r);
    let limit = 8 * g.n;
    for _ in 0..limit {
        g.apply_a(&d, &mut ax);
        let denom = dot(&d, &ax);
        if denom <= 0.0 {
            return Err("velocity CG lost positive definiteness".into());
        }
        let alpha = rr / denom;
        for k in 0..g.len {
            x[k] += alpha * d[k];
            r[k] -= alpha * ax[k];
        }
        c.inner_iters += 1;
        let new_rr = dot(&r, &r);
        if new_rr.sqrt() <= target {
            return Ok(());
        }
        let beta = new_rr / rr;
        for k in 0..g.len {
            d[k] = r[k] + beta * d[k];
        }
        rr = new_rr;
    }
    Err(format!(
        "velocity CG did not converge: residual={:.3e}, target={:.3e}",
        rr.sqrt(),
        target
    ))
}

fn apply_s(
    g: &Grid,
    backend: &Backend,
    p: &[f64],
    out: &mut [f64],
    c: &mut Counters,
) -> Result<(), String> {
    let mut grad = vec![0.0; g.len];
    let mut ainvg = vec![0.0; g.len];
    g.gradient(p, &mut grad);
    solve_velocity(g, backend, &grad, &mut ainvg, c)?;
    g.divergence(&ainvg, out);
    for a in out.iter_mut() {
        *a = -*a;
    }
    zero_mean(out);
    Ok(())
}

fn solve_stokes(
    g: &Grid,
    backend: &Backend,
    f: &[f64],
    initial: Option<(&[f64], &[f64])>,
) -> Result<(Vec<f64>, Vec<f64>, Counters), String> {
    let mut c = Counters {
        inner_iters: 0,
        outer_iters: 0,
        inverse_calls: 0,
    };
    let mut aif = initial.map_or_else(|| vec![0.0; g.len], |(u, _)| u.to_vec());
    solve_velocity(g, backend, f, &mut aif, &mut c)?;
    let mut rhs = vec![0.0; g.n * g.n];
    g.divergence(&aif, &mut rhs);
    for a in rhs.iter_mut() {
        *a = -*a;
    }
    zero_mean(&mut rhs);
    let mut p = initial.map_or_else(|| vec![0.0; rhs.len()], |(_, q)| q.to_vec());
    zero_mean(&mut p);
    let mut sp = vec![0.0; rhs.len()];
    if initial.is_some() {
        apply_s(g, backend, &p, &mut sp, &mut c)?;
    }
    let mut r: Vec<f64> = rhs.iter().zip(&sp).map(|(b, a)| b - a).collect();
    zero_mean(&mut r);
    let target = 2e-9 * norm(&rhs).max(1.0);
    let mut d = r.clone();
    let mut rr = dot(&r, &r);
    while rr.sqrt() > target {
        if c.outer_iters >= 6 * g.n {
            return Err(format!(
                "Schur CG did not converge: residual={:.3e}, target={:.3e}",
                rr.sqrt(),
                target
            ));
        }
        apply_s(g, backend, &d, &mut sp, &mut c)?;
        let denom = dot(&d, &sp);
        if denom <= 0.0 {
            return Err(format!("Schur CG lost positive definiteness: {denom:.3e}"));
        }
        let alpha = rr / denom;
        for k in 0..p.len() {
            p[k] += alpha * d[k];
            r[k] -= alpha * sp[k];
        }
        zero_mean(&mut r);
        c.outer_iters += 1;
        let new_rr = dot(&r, &r);
        if new_rr.sqrt() <= target {
            break;
        }
        let beta = new_rr / rr;
        for k in 0..d.len() {
            d[k] = r[k] + beta * d[k];
        }
        zero_mean(&mut d);
        rr = new_rr;
    }
    zero_mean(&mut p);
    let mut grad = vec![0.0; g.len];
    g.gradient(&p, &mut grad);
    let mut adjusted: Vec<f64> = f.iter().zip(&grad).map(|(fi, gi)| fi - gi).collect();
    let mut u = initial.map_or_else(|| vec![0.0; g.len], |(u, _)| u.to_vec());
    solve_velocity(g, backend, &adjusted, &mut u, &mut c)?;
    adjusted.clear();
    Ok((u, p, c))
}

const RADIUS: f64 = 0.15;
const BLOB_RADIUS: f64 = 0.075;
const TARGET_U: f64 = 0.1;

#[derive(Clone, Copy)]
enum Shape {
    Circle,
    Blob,
}

impl Shape {
    fn name(self) -> &'static str {
        match self {
            Self::Circle => "circle",
            Self::Blob => "blob",
        }
    }
    fn parameter(self) -> &'static str {
        match self {
            Self::Circle => "kappa_per_length",
            Self::Blob => "gamma_total",
        }
    }
    fn point_resistance(self, m: usize, parameter: f64) -> f64 {
        match self {
            Self::Circle => parameter * 2.0 * PI * RADIUS / m as f64,
            Self::Blob => parameter / m as f64,
        }
    }
}

struct Marker {
    x: f64,
    y: f64,
    ux: Vec<(usize, f64)>,
    uy: Vec<(usize, f64)>,
}

fn peskin(r: f64) -> f64 {
    let r = r.abs();
    if r < 1.0 {
        (3.0 - 2.0 * r + (1.0 + 4.0 * r - 4.0 * r * r).sqrt()) / 8.0
    } else if r < 2.0 {
        (5.0 - 2.0 * r - (-7.0 + 12.0 * r - 4.0 * r * r).sqrt()) / 8.0
    } else {
        0.0
    }
}

fn stencil(g: &Grid, x: f64, y: f64, is_u: bool) -> Vec<(usize, f64)> {
    let xgrid = x / g.h - if is_u { 0.0 } else { 0.5 };
    let ygrid = y / g.h - if is_u { 0.5 } else { 0.0 };
    let mut out = Vec::with_capacity(16);
    let xi = xgrid.floor() as isize;
    let yi = ygrid.floor() as isize;
    for j in yi - 1..=yi + 2 {
        if (is_u && (j < 0 || j >= g.n as isize)) || (!is_u && (j < 1 || j >= g.n as isize)) {
            continue;
        }
        for i in xi - 1..=xi + 2 {
            let w = peskin(xgrid - i as f64) * peskin(ygrid - j as f64);
            let ii = i.rem_euclid(g.n as isize) as usize;
            let idx = if is_u {
                g.u(ii, j as usize)
            } else {
                g.v(ii, j as usize)
            };
            out.push((idx, w));
        }
    }
    out
}

fn point(g: &Grid, x: f64, y: f64) -> Marker {
    Marker {
        x,
        y,
        ux: stencil(g, x, y, true),
        uy: stencil(g, x, y, false),
    }
}

fn circle(g: &Grid, m: usize) -> Vec<Marker> {
    (0..m)
        .map(|a| {
            let angle = 2.0 * PI * a as f64 / m as f64;
            point(g, 0.5 + RADIUS * angle.cos(), 0.5 + RADIUS * angle.sin())
        })
        .collect()
}

fn blob(g: &Grid, m: usize) -> Vec<Marker> {
    match m {
        1 => vec![point(g, 0.5, 0.5)],
        4 => (0..4)
            .map(|a| {
                let angle = 2.0 * PI * a as f64 / 4.0;
                point(
                    g,
                    0.5 + BLOB_RADIUS * angle.cos(),
                    0.5 + BLOB_RADIUS * angle.sin(),
                )
            })
            .collect(),
        _ => unreachable!("blob count checked by CLI"),
    }
}

fn sample(markers: &[Marker], u: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; 2 * markers.len()];
    for (a, marker) in markers.iter().enumerate() {
        out[2 * a] = marker.ux.iter().map(|(idx, w)| w * u[*idx]).sum();
        out[2 * a + 1] = marker.uy.iter().map(|(idx, w)| w * u[*idx]).sum();
    }
    out
}

fn spread(g: &Grid, markers: &[Marker], force: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; g.len];
    let inv_area = 1.0 / (g.h * g.h);
    for (a, marker) in markers.iter().enumerate() {
        for &(idx, w) in &marker.ux {
            out[idx] += force[2 * a] * w * inv_area;
        }
        for &(idx, w) in &marker.uy {
            out[idx] += force[2 * a + 1] * w * inv_area;
        }
    }
    out
}

fn target(m: usize) -> Vec<f64> {
    (0..m).flat_map(|_| [TARGET_U, 0.0]).collect()
}

fn solve_dense_spd(a: &[f64], b: &[f64], dim: usize) -> Result<Vec<f64>, String> {
    let mut l = vec![0.0; dim * dim];
    for i in 0..dim {
        for j in 0..=i {
            let mut value = 0.5 * (a[i * dim + j] + a[j * dim + i]);
            for k in 0..j {
                value -= l[i * dim + k] * l[j * dim + k];
            }
            if i == j {
                if value <= 0.0 {
                    return Err(format!("mobility system not SPD at {i}: {value:e}"));
                }
                l[i * dim + j] = value.sqrt();
            } else {
                l[i * dim + j] = value / l[j * dim + j];
            }
        }
    }
    let mut y = vec![0.0; dim];
    for i in 0..dim {
        y[i] = b[i];
        for j in 0..i {
            y[i] -= l[i * dim + j] * y[j];
        }
        y[i] /= l[i * dim + i];
    }
    let mut x = vec![0.0; dim];
    for i in (0..dim).rev() {
        x[i] = y[i];
        for j in i + 1..dim {
            x[i] -= l[j * dim + i] * x[j];
        }
        x[i] /= l[i * dim + i];
    }
    Ok(x)
}

fn rms(x: &[f64]) -> f64 {
    (dot(x, x) / x.len() as f64).sqrt()
}

fn inspect(
    g: &Grid,
    markers: &[Marker],
    shape: Shape,
    point_resistance: f64,
    force: &[f64],
    u: &[f64],
    p: &[f64],
    f: &[f64],
) -> (f64, f64, f64, f64, f64, f64, f64, f64, f64, f64, f64) {
    let m = markers.len();
    let sampled = sample(markers, u);
    let desired = target(m);
    let slip: Vec<f64> = sampled.iter().zip(&desired).map(|(a, b)| a - b).collect();
    let law: Vec<f64> = (0..2 * m)
        .map(|i| force[i] / point_resistance + sampled[i] - desired[i])
        .collect();
    let (normal_abs, tangent_abs) = if matches!(shape, Shape::Circle) {
        let dense = circle(g, 720);
        let dense_u = sample(&dense, u);
        let mut normal_abs = 0.0;
        let mut tangent_abs = 0.0;
        for i in 0..dense.len() {
            let theta = 2.0 * PI * i as f64 / dense.len() as f64;
            let nx = theta.cos();
            let ny = theta.sin();
            let dx = dense_u[2 * i] - TARGET_U;
            let dy = dense_u[2 * i + 1];
            normal_abs += (dx * nx + dy * ny).abs() * 2.0 * PI * RADIUS / dense.len() as f64;
            tangent_abs += (-dx * ny + dy * nx).abs() * 2.0 * PI * RADIUS / dense.len() as f64;
        }
        (normal_abs, tangent_abs)
    } else {
        (f64::NAN, f64::NAN)
    };
    let reaction_x: f64 = -(0..m).map(|i| force[2 * i]).sum::<f64>();
    let reaction_y: f64 = -(0..m).map(|i| force[2 * i + 1]).sum::<f64>();
    let torque: f64 = -(0..m)
        .map(|i| (markers[i].x - 0.5) * force[2 * i + 1] - (markers[i].y - 0.5) * force[2 * i])
        .sum::<f64>();
    let power_marker = dot(force, &sampled);
    let power_grid = dot(f, u) * g.h * g.h;
    let adjoint_error = (power_grid - power_marker).abs() / power_marker.abs().max(1e-15);
    let mut div = vec![0.0; g.n * g.n];
    g.divergence(u, &mut div);
    let mut au = vec![0.0; g.len];
    g.apply_a(u, &mut au);
    let mut gp = vec![0.0; g.len];
    g.gradient(p, &mut gp);
    let momentum: Vec<f64> = (0..g.len).map(|i| au[i] + gp[i] - f[i]).collect();
    let work_input = dot(force, &desired);
    let viscous = dot(u, &au) * g.h * g.h;
    let slip_diss = dot(force, force) / point_resistance;
    let energy_error = (work_input - viscous - slip_diss).abs() / work_input.abs().max(1e-15);
    (
        rms(&slip),
        rms(&law),
        normal_abs,
        tangent_abs,
        reaction_x,
        reaction_y,
        torque,
        adjoint_error,
        rms(&div),
        rms(&momentum),
        energy_error,
    )
}

fn export_fields(g: &Grid, u: &[f64], p: &[f64], path: &Path) -> Result<(), String> {
    let file = File::create(path).map_err(|e| e.to_string())?;
    let mut out = BufWriter::new(file);
    writeln!(out, "x,y,u,v,p").map_err(|e| e.to_string())?;
    for j in 0..g.n {
        for i in 0..g.n {
            let ux = 0.5 * (u[g.u(i, j)] + u[g.u((i + 1) % g.n, j)]);
            let vdown = if j == 0 { 0.0 } else { u[g.v(i, j)] };
            let vup = if j + 1 == g.n { 0.0 } else { u[g.v(i, j + 1)] };
            writeln!(
                out,
                "{:.12},{:.12},{:.12e},{:.12e},{:.12e}",
                (i as f64 + 0.5) * g.h,
                (j as f64 + 0.5) * g.h,
                ux,
                0.5 * (vdown + vup),
                p[g.p(i, j)]
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn experiment(
    n: usize,
    m: usize,
    shape: Shape,
    parameter: f64,
    outdir: &Path,
    lagged: bool,
) -> Result<(), String> {
    let total_start = Instant::now();
    let g = Grid::new(n);
    let backend = Backend::new("separable", n)?;
    let markers = match shape {
        Shape::Circle => circle(&g, m),
        Shape::Blob => blob(&g, m),
    };
    for marker in &markers {
        for weights in [&marker.ux, &marker.uy] {
            let sum: f64 = weights.iter().map(|(_, w)| w).sum();
            if (sum - 1.0).abs() > 1e-12 {
                return Err(format!("marker stencil lost partition of unity: {sum:e}"));
            }
        }
    }
    let dim = 2 * m;
    let point_resistance = shape.point_resistance(m, parameter);
    let mut calls = 0usize;
    let assemble_start = Instant::now();
    let mut mobility = vec![0.0; dim * dim];
    for col in 0..dim {
        let mut basis = vec![0.0; dim];
        basis[col] = 1.0;
        let f = spread(&g, &markers, &basis);
        let (u, _, _) = solve_stokes(&g, &backend, &f, None)?;
        calls += 1;
        let sampled = sample(&markers, &u);
        for row in 0..dim {
            mobility[row * dim + col] = sampled[row];
        }
    }
    let assemble_ms = assemble_start.elapsed().as_secs_f64() * 1000.0;
    let anti = (0..dim)
        .flat_map(|i| (0..dim).map(move |j| (i, j)))
        .map(|(i, j)| (mobility[i * dim + j] - mobility[j * dim + i]).powi(2))
        .sum::<f64>()
        .sqrt();
    let base = mobility.iter().map(|x| x * x).sum::<f64>().sqrt();
    let anti_rel = anti / base;
    if anti_rel > 1e-6 {
        return Err(format!("mobility antisymmetry too large: {anti_rel:e}"));
    }
    let factor_start = Instant::now();
    let mut system = mobility;
    for i in 0..dim {
        system[i * dim + i] += 1.0 / point_resistance;
    }
    let force = solve_dense_spd(&system, &target(m), dim)?;
    let factor_ms = factor_start.elapsed().as_secs_f64() * 1000.0;
    let final_start = Instant::now();
    let f = spread(&g, &markers, &force);
    let (u, p, c) = solve_stokes(&g, &backend, &f, None)?;
    calls += 1;
    let final_ms = final_start.elapsed().as_secs_f64() * 1000.0;
    let total_ms = total_start.elapsed().as_secs_f64() * 1000.0;
    let stats = inspect(&g, &markers, shape, point_resistance, &force, &u, &p, &f);
    let force_balance_x = (f.iter().take(n * n).sum::<f64>() * g.h * g.h + stats.4).abs();
    let force_balance_y = (f.iter().skip(n * n).sum::<f64>() * g.h * g.h + stats.5).abs();
    let field_path = outdir.join(format!("field_{}_n{n}_m{m}_p{parameter}.csv", shape.name()));
    export_fields(&g, &u, &p, &field_path)?;
    let speed_ratio = sample(&markers, &u)
        .chunks_exact(2)
        .map(|v| v[0].hypot(v[1]) / TARGET_U)
        .fold(f64::NEG_INFINITY, f64::max);
    println!("self,{},{n},{m},{},{parameter},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{calls},{},{:.6},{:.6},{:.6},{:.6},{:.9e}",shape.name(),shape.parameter(),stats.0,stats.1,stats.2,stats.3,stats.4,stats.5,stats.6,stats.7,stats.8,stats.9,stats.10,anti_rel,force_balance_x.max(force_balance_y),c.outer_iters,assemble_ms,factor_ms,final_ms,total_ms,speed_ratio);
    if lagged {
        let lagged_start = Instant::now();
        let force_lag: Vec<f64> = target(m)
            .into_iter()
            .map(|v| v * point_resistance)
            .collect();
        let f_lag = spread(&g, &markers, &force_lag);
        let (u_lag, p_lag, c_lag) = solve_stokes(&g, &backend, &f_lag, None)?;
        let lagged_ms = lagged_start.elapsed().as_secs_f64() * 1000.0;
        let s = inspect(
            &g,
            &markers,
            shape,
            point_resistance,
            &force_lag,
            &u_lag,
            &p_lag,
            &f_lag,
        );
        let j = sample(&markers, &u_lag);
        let lag_speed_ratio = j
            .chunks_exact(2)
            .map(|v| v[0].hypot(v[1]) / TARGET_U)
            .fold(f64::NEG_INFINITY, f64::max);
        let lag_fx = (f_lag.iter().take(n * n).sum::<f64>() * g.h * g.h + s.4).abs();
        let lag_fy = (f_lag.iter().skip(n * n).sum::<f64>() * g.h * g.h + s.5).abs();
        println!("lagged,{},{n},{m},{},{parameter},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},NaN,{:.9e},1,{},{:.6},{:.6},{:.6},{:.6},{:.9e}",shape.name(),shape.parameter(),s.0,s.1,s.2,s.3,s.4,s.5,s.6,s.7,s.8,s.9,s.10,lag_fx.max(lag_fy),c_lag.outer_iters,0.0,0.0,lagged_ms,lagged_ms,lag_speed_ratio);
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 8 || args[4] != "--shape" {
        return Err("usage: marker-probe N M OUTPUT_DIR --shape circle --kappa KAPPA\n       marker-probe N M OUTPUT_DIR --shape blob --drag GAMMA".into());
    }
    let n = args[1].parse::<usize>().map_err(|e| e.to_string())?;
    let m = args[2].parse::<usize>().map_err(|e| e.to_string())?;
    let outdir = Path::new(&args[3]);
    let shape = match args[5].as_str() {
        "circle" if args[6] == "--kappa" => Shape::Circle,
        "blob" if args[6] == "--drag" && (m == 1 || m == 4) => Shape::Blob,
        "blob" => return Err("blob requires M=1 or M=4 and --drag GAMMA".into()),
        _ => return Err("circle requires --kappa; blob requires --drag".into()),
    };
    let parameter = args[7].parse::<f64>().map_err(|e| e.to_string())?;
    if n < 8 || m == 0 || !parameter.is_finite() || parameter <= 0.0 {
        return Err("require N>=8, M>0, and finite positive resistance".into());
    }
    fs::create_dir_all(outdir).map_err(|e| e.to_string())?;
    println!("kind,shape,n,m,parameter_kind,parameter,slip_rms,force_law_rms,normal_abs_integral,tangent_abs_integral,reaction_x,reaction_y,reaction_torque,adjoint_rel,div_rms,momentum_rms,energy_rel,mobility_antisym_rel,force_balance_abs,fluid_calls,pressure_iterations_final,assemble_ms,factor_ms,final_fluid_ms,total_ms,max_sampled_speed_over_target");
    experiment(n, m, shape, parameter, outdir, true)
}
