//! Throwaway periodic-channel MAC/Stokes finite-drag research solver.
//! x periodic, y no-slip, unit square, viscosity 1.
#[path = "separable.rs"]
mod separable;
use rayon::ThreadPool;
use separable::SeparableInverse;
use std::time::Instant;

/// Inclusive inverse wall time contains the four inverse phase times.
#[derive(Clone, Copy, Debug, Default)]
pub struct Profile {
    pub inverse_calls: usize,
    pub velocity_inverse_ms: f64,
    pub inverse_prepare_copy_ms: f64,
    pub forward_fft_ms: f64,
    pub mode_gather_solve_scatter_ms: f64,
    pub inverse_fft_ms: f64,
    pub pressure_stencil_ms: f64,
}

fn pressure_work<F: FnOnce()>(profiling: bool, profile: &mut Profile, work: F) {
    let start = profiling.then(Instant::now);
    work();
    if let Some(start) = start {
        profile.pressure_stencil_ms += start.elapsed().as_secs_f64() * 1000.0;
    }
}

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
    Separable(SeparableInverse),
}
impl Backend {
    fn new(name: &str, n: usize) -> Result<Self, String> {
        match name {
            "separable" => Ok(Self::Separable(SeparableInverse::new(n)?)),
            _ => Err(format!("invalid backend: {name}")),
        }
    }

    fn with_threads(n: usize, threads: usize) -> Result<Self, String> {
        Ok(Self::Separable(SeparableInverse::with_threads(n, threads)?))
    }

    fn take_pool(&mut self) -> Option<ThreadPool> {
        match self {
            Self::Separable(inv) => inv.take_pool(),
        }
    }

    fn restore_pool(&mut self, pool: Option<ThreadPool>) {
        match self {
            Self::Separable(inv) => inv.restore_pool(pool),
        }
    }
}

struct Counters {
    outer_iters: usize,
    inverse_calls: usize,
}

fn solve_velocity(
    _g: &Grid,
    backend: &mut Backend,
    b: &[f64],
    x: &mut [f64],
    c: &mut Counters,
    profiling: bool,
    profile: &mut Profile,
) -> Result<(), String> {
    c.inverse_calls += 1;
    let started = profiling.then(Instant::now);
    match backend {
        Backend::Separable(inv) => {
            inv.solve(b, x, profiling, profile);
            if let Some(started) = started {
                profile.inverse_calls += 1;
                profile.velocity_inverse_ms += started.elapsed().as_secs_f64() * 1000.0;
            }
            Ok(())
        }
    }
}

fn apply_s(
    g: &Grid,
    backend: &mut Backend,
    p: &[f64],
    out: &mut [f64],
    grad: &mut [f64],
    ainvg: &mut [f64],
    c: &mut Counters,
    profiling: bool,
    profile: &mut Profile,
) -> Result<(), String> {
    pressure_work(profiling, profile, || g.gradient(p, grad));
    solve_velocity(g, backend, grad, ainvg, c, profiling, profile)?;
    pressure_work(profiling, profile, || g.divergence(ainvg, out));
    for a in out.iter_mut() {
        *a = -*a;
    }
    zero_mean(out);
    Ok(())
}

fn solve_stokes(
    g: &Grid,
    backend: &mut Backend,
    f: &[f64],
    initial: Option<(&[f64], &[f64])>,
    profiling: bool,
    profile: &mut Profile,
) -> Result<(Vec<f64>, Vec<f64>, Counters), String> {
    let mut c = Counters {
        outer_iters: 0,
        inverse_calls: 0,
    };
    let mut aif = initial.map_or_else(|| vec![0.0; g.len], |(u, _)| u.to_vec());
    solve_velocity(g, backend, f, &mut aif, &mut c, profiling, profile)?;
    let mut rhs = vec![0.0; g.n * g.n];
    pressure_work(profiling, profile, || g.divergence(&aif, &mut rhs));
    for a in rhs.iter_mut() {
        *a = -*a;
    }
    zero_mean(&mut rhs);
    let mut p = initial.map_or_else(|| vec![0.0; rhs.len()], |(_, q)| q.to_vec());
    zero_mean(&mut p);
    let mut sp = vec![0.0; rhs.len()];
    // These arrays live for the entire pressure CG, instead of allocating on
    // every Schur operator application.
    let mut schur_grad = vec![0.0; g.len];
    let mut schur_ainvg = vec![0.0; g.len];
    if initial.is_some() {
        apply_s(
            g,
            backend,
            &p,
            &mut sp,
            &mut schur_grad,
            &mut schur_ainvg,
            &mut c,
            profiling,
            profile,
        )?;
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
        apply_s(
            g,
            backend,
            &d,
            &mut sp,
            &mut schur_grad,
            &mut schur_ainvg,
            &mut c,
            profiling,
            profile,
        )?;
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
    pressure_work(profiling, profile, || g.gradient(&p, &mut schur_grad));
    let mut adjusted: Vec<f64> = f.iter().zip(&schur_grad).map(|(fi, gi)| fi - gi).collect();
    let mut u = initial.map_or_else(|| vec![0.0; g.len], |(u, _)| u.to_vec());
    solve_velocity(g, backend, &adjusted, &mut u, &mut c, profiling, profile)?;
    adjusted.clear();
    Ok((u, p, c))
}

#[derive(Clone, Copy, Debug)]
pub struct Marker {
    pub position: [f64; 2],
    pub velocity: [f64; 2],
    /// Point drag; four points of a cell each use 0.0625 for total 0.25.
    pub drag: f64,
}
struct PreparedMarker {
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

fn point(g: &Grid, x: f64, y: f64) -> PreparedMarker {
    PreparedMarker {
        ux: stencil(g, x, y, true),
        uy: stencil(g, x, y, false),
    }
}

fn sample(markers: &[PreparedMarker], u: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; 2 * markers.len()];
    for (a, marker) in markers.iter().enumerate() {
        out[2 * a] = marker.ux.iter().map(|(idx, w)| w * u[*idx]).sum();
        out[2 * a + 1] = marker.uy.iter().map(|(idx, w)| w * u[*idx]).sum();
    }
    out
}

fn spread(g: &Grid, markers: &[PreparedMarker], force: &[f64]) -> Vec<f64> {
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

#[derive(Debug)]
pub struct Solution {
    pub grid_velocity: Vec<f64>,
    pub pressure: Vec<f64>,
    /// Force exerted by each marker on the fluid. The cell feels its negative.
    pub marker_forces: Vec<[f64; 2]>,
    pub sampled_velocity: Vec<[f64; 2]>,
    pub coupling_iterations: usize,
    pub fluid_calls: usize,
    pub pressure_iterations: usize,
    pub divergence_rms: f64,
    pub momentum_rms: f64,
    /// RMS of F/gamma + Ju - V, in velocity units.
    pub force_law_rms: f64,
    /// Maximum absolute component of F/drag + Ju - V, in velocity units.
    pub force_law_max_component_abs: f64,
    /// Sum of F dot (Ju - V) using the final fluid field; may be positive.
    pub exchange_power: f64,
    /// Lagged/relaxed force Vec allocation and per-point trial/mix loop only.
    /// Self-consistent CG reports zero because it uses a different update.
    pub point_force_update_ms: f64,
    pub grid_force_balance_abs: f64,
    /// Smallest sum of weights on any velocity component; below 1 means wall clipping.
    pub min_kernel_weight_sum: f64,
    pub kernel_prepare_ms: f64,
    pub coupled_solve_ms: f64,
    pub diagnostics_ms: f64,
}

pub struct Stokes {
    grid: Grid,
    backend: Backend,
    profiling: bool,
    profile: Profile,
}

impl Stokes {
    pub fn new(n: usize) -> Self {
        Self::try_new(n).expect("Stokes grid must be a power of two >= 2")
    }

    pub fn try_new(n: usize) -> Result<Self, String> {
        let backend = Backend::new("separable", n)?;
        Ok(Self {
            grid: Grid::new(n),
            backend,
            profiling: false,
            profile: Profile::default(),
        })
    }

    /// `threads=0` preserves the original serial FFT/Thomas layout and loops.
    /// `threads=1/2/4` owns a persistent Rayon pool for the separable inverse.
    pub fn with_threads(n: usize, threads: usize) -> Result<Self, String> {
        let backend = Backend::with_threads(n, threads)?;
        Ok(Self {
            grid: Grid::new(n),
            backend,
            profiling: false,
            profile: Profile::default(),
        })
    }

    pub fn set_profiling(&mut self, enabled: bool) {
        self.profiling = enabled;
        self.profile = Profile::default();
    }

    pub fn take_profile(&mut self) -> Profile {
        std::mem::take(&mut self.profile)
    }

    pub fn sample_at(&self, solution: &Solution, position: [f64; 2]) -> [f64; 2] {
        let marker = point(&self.grid, position[0], position[1]);
        let sampled = sample(&[marker], &solution.grid_velocity);
        [sampled[0], sampled[1]]
    }

    pub fn solve(
        &mut self,
        markers: &[Marker],
        background_force: [f64; 2],
    ) -> Result<Solution, String> {
        let pool = self.backend.take_pool();
        let result = if let Some(pool_ref) = pool.as_ref() {
            pool_ref.install(|| self.solve_inner(markers, background_force))
        } else {
            self.solve_inner(markers, background_force)
        };
        self.backend.restore_pool(pool);
        result
    }

    fn solve_inner(
        &mut self,
        markers: &[Marker],
        background_force: [f64; 2],
    ) -> Result<Solution, String> {
        let prepare_start = Instant::now();
        let g = &self.grid;
        if background_force.iter().any(|x| !x.is_finite()) {
            return Err("background force must be finite".into());
        }
        for (i, m) in markers.iter().enumerate() {
            if !m
                .position
                .iter()
                .chain(m.velocity.iter())
                .all(|x| x.is_finite())
                || !m.drag.is_finite()
                || m.drag <= 0.0
                || !(0.0..=1.0).contains(&m.position[1])
            {
                return Err(format!("invalid marker {i}"));
            }
        }
        let prepared: Vec<_> = markers
            .iter()
            .map(|m| point(g, m.position[0], m.position[1]))
            .collect();
        let min_kernel_weight_sum = prepared
            .iter()
            .flat_map(|m| [&m.ux, &m.uy])
            .map(|weights| weights.iter().map(|(_, w)| w).sum::<f64>())
            .fold(1.0_f64, f64::min);
        if min_kernel_weight_sum < 1.0 - 1e-10 {
            return Err(format!("marker Peskin kernel clipped by no-slip wall: min weight sum={min_kernel_weight_sum:.9}"));
        }
        let kernel_prepare_ms = prepare_start.elapsed().as_secs_f64() * 1000.0;
        let coupled_start = Instant::now();
        let mut background = vec![0.0; g.len];
        for (i, b) in background.iter_mut().enumerate() {
            *b = background_force[usize::from(i >= g.n * g.n)];
        }
        let (u0, p0, c0) = solve_stokes(
            g,
            &mut self.backend,
            &background,
            None,
            self.profiling,
            &mut self.profile,
        )?;
        let j0 = sample(&prepared, &u0);
        let dim = markers.len() * 2;
        let mut rhs = vec![0.0; dim];
        for (i, m) in markers.iter().enumerate() {
            rhs[2 * i] = m.velocity[0] - j0[2 * i];
            rhs[2 * i + 1] = m.velocity[1] - j0[2 * i + 1];
        }
        // (diag(1/gamma) + J K S) F = V - J K f_background.
        // K is the complete divergence-free Stokes response, not a projection step.
        let mut force = vec![0.0; dim];
        let mut r = rhs.clone();
        let mut direction = r.clone();
        let mut rr = dot(&r, &r);
        let target = 1e-9 * norm(&rhs).max(1e-4);
        let mut coupling_iterations = 0;
        let mut fluid_calls = 1;
        let mut pressure_iterations = c0.outer_iters;
        while rr.sqrt() > target {
            if coupling_iterations >= (4 * dim).max(20) {
                return Err(format!(
                    "finite-drag CG did not converge: residual={:.3e}, target={target:.3e}",
                    rr.sqrt()
                ));
            }
            let spread_direction = spread(g, &prepared, &direction);
            let (response, _, c) = solve_stokes(
                g,
                &mut self.backend,
                &spread_direction,
                None,
                self.profiling,
                &mut self.profile,
            )?;
            fluid_calls += 1;
            pressure_iterations += c.outer_iters;
            let mut ad = sample(&prepared, &response);
            for (i, m) in markers.iter().enumerate() {
                ad[2 * i] += direction[2 * i] / m.drag;
                ad[2 * i + 1] += direction[2 * i + 1] / m.drag;
            }
            let denominator = dot(&direction, &ad);
            if denominator <= 0.0 || !denominator.is_finite() {
                return Err(format!(
                    "finite-drag operator lost positive definiteness: {denominator:.3e}"
                ));
            }
            let alpha = rr / denominator;
            for i in 0..dim {
                force[i] += alpha * direction[i];
                r[i] -= alpha * ad[i];
            }
            coupling_iterations += 1;
            let new_rr = dot(&r, &r);
            if new_rr.sqrt() <= target {
                break;
            }
            let beta = new_rr / rr;
            for i in 0..dim {
                direction[i] = r[i] + beta * direction[i];
            }
            rr = new_rr;
        }
        let marker_grid_force = spread(g, &prepared, &force);
        let full_force: Vec<f64> = background
            .iter()
            .zip(&marker_grid_force)
            .map(|(a, b)| a + b)
            .collect();
        let (grid_velocity, pressure) = if dim == 0 {
            (u0, p0)
        } else {
            let (u, p, count) = solve_stokes(
                g,
                &mut self.backend,
                &full_force,
                None,
                self.profiling,
                &mut self.profile,
            )?;
            fluid_calls += 1;
            pressure_iterations += count.outer_iters;
            (u, p)
        };
        let coupled_solve_ms = coupled_start.elapsed().as_secs_f64() * 1000.0;
        let diagnostics_start = Instant::now();
        let sampled = sample(&prepared, &grid_velocity);
        let marker_forces = force.chunks_exact(2).map(|f| [f[0], f[1]]).collect();
        let sampled_velocity = sampled.chunks_exact(2).map(|u| [u[0], u[1]]).collect();
        let mut closure = vec![0.0; dim];
        for (i, m) in markers.iter().enumerate() {
            closure[2 * i] = force[2 * i] / m.drag + sampled[2 * i] - m.velocity[0];
            closure[2 * i + 1] = force[2 * i + 1] / m.drag + sampled[2 * i + 1] - m.velocity[1];
        }
        let force_law_rms = if dim == 0 {
            0.0
        } else {
            norm(&closure) / (dim as f64).sqrt()
        };
        let force_law_max_component_abs = closure.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
        let exchange_power: f64 = markers
            .iter()
            .enumerate()
            .map(|(i, m)| {
                force[2 * i] * (sampled[2 * i] - m.velocity[0])
                    + force[2 * i + 1] * (sampled[2 * i + 1] - m.velocity[1])
            })
            .sum();
        let mut divergence = vec![0.0; g.n * g.n];
        g.divergence(&grid_velocity, &mut divergence);
        let divergence_rms = norm(&divergence) / (divergence.len() as f64).sqrt();
        let mut au = vec![0.0; g.len];
        let mut gp = vec![0.0; g.len];
        g.apply_a(&grid_velocity, &mut au);
        g.gradient(&pressure, &mut gp);
        let momentum_rms = au
            .iter()
            .zip(&gp)
            .zip(&full_force)
            .map(|((a, p), f)| (a + p - f).powi(2))
            .sum::<f64>()
            .sqrt()
            / (g.len as f64).sqrt();
        let marker_sum = [
            force.iter().step_by(2).sum::<f64>(),
            force.iter().skip(1).step_by(2).sum::<f64>(),
        ];
        let grid_sum = [
            marker_grid_force[..g.n * g.n].iter().sum::<f64>() * g.h * g.h,
            marker_grid_force[g.n * g.n..].iter().sum::<f64>() * g.h * g.h,
        ];
        let grid_force_balance_abs =
            ((marker_sum[0] - grid_sum[0]).powi(2) + (marker_sum[1] - grid_sum[1]).powi(2)).sqrt();
        let diagnostics_ms = diagnostics_start.elapsed().as_secs_f64() * 1000.0;
        Ok(Solution {
            grid_velocity,
            pressure,
            marker_forces,
            sampled_velocity,
            coupling_iterations,
            fluid_calls,
            pressure_iterations,
            divergence_rms,
            momentum_rms,
            force_law_rms,
            force_law_max_component_abs,
            exchange_power,
            point_force_update_ms: 0.0,
            grid_force_balance_abs,
            min_kernel_weight_sum,
            kernel_prepare_ms,
            coupled_solve_ms,
            diagnostics_ms,
        })
    }

    /// One lagged finite-drag update. With a prior field this performs one
    /// complete Stokes solve; without one it first solves the unladen background.
    /// The returned force-law residual is evaluated using the *new* field.
    pub fn solve_lagged(
        &mut self,
        markers: &[Marker],
        background_force: [f64; 2],
        prior: Option<&Solution>,
    ) -> Result<Solution, String> {
        let pool = self.backend.take_pool();
        let result = if let Some(pool_ref) = pool.as_ref() {
            pool_ref.install(|| self.solve_lagged_inner(markers, background_force, prior, 1.0))
        } else {
            self.solve_lagged_inner(markers, background_force, prior, 1.0)
        };
        self.backend.restore_pool(pool);
        result
    }

    /// Relax only the marker force. The same mixed force is spread to the grid
    /// and returned to the caller for the opposite mechanical impulse.
    /// Marker order must preserve identity across calls; a changed count is rejected.
    pub fn solve_relaxed(
        &mut self,
        markers: &[Marker],
        background_force: [f64; 2],
        prior: Option<&Solution>,
        omega: f64,
    ) -> Result<Solution, String> {
        if !omega.is_finite() || omega <= 0.0 || omega > 1.0 {
            return Err("omega must be finite and in (0, 1]".into());
        }
        if let Some(p) = prior {
            if p.marker_forces.len() != markers.len() {
                return Err(format!(
                    "prior marker count {} differs from current {}",
                    p.marker_forces.len(),
                    markers.len()
                ));
            }
        }
        if omega == 1.0 {
            return self.solve_lagged(markers, background_force, prior);
        }
        let pool = self.backend.take_pool();
        let result = if let Some(pool_ref) = pool.as_ref() {
            pool_ref.install(|| self.solve_lagged_inner(markers, background_force, prior, omega))
        } else {
            self.solve_lagged_inner(markers, background_force, prior, omega)
        };
        self.backend.restore_pool(pool);
        result
    }

    fn solve_lagged_inner(
        &mut self,
        markers: &[Marker],
        background_force: [f64; 2],
        prior: Option<&Solution>,
        omega: f64,
    ) -> Result<Solution, String> {
        let prepare_start = Instant::now();
        let g = &self.grid;
        if background_force.iter().any(|x| !x.is_finite()) {
            return Err("background force must be finite".into());
        }
        for (i, m) in markers.iter().enumerate() {
            if !m
                .position
                .iter()
                .chain(m.velocity.iter())
                .all(|x| x.is_finite())
                || !m.drag.is_finite()
                || m.drag <= 0.0
                || !(0.0..=1.0).contains(&m.position[1])
            {
                return Err(format!("invalid marker {i}"));
            }
        }
        if let Some(p) = prior {
            if p.grid_velocity.len() != g.len {
                return Err("prior grid size differs".into());
            }
        }
        let prepared: Vec<_> = markers
            .iter()
            .map(|m| point(g, m.position[0], m.position[1]))
            .collect();
        let min_kernel_weight_sum = prepared
            .iter()
            .flat_map(|m| [&m.ux, &m.uy])
            .map(|weights| weights.iter().map(|(_, w)| w).sum::<f64>())
            .fold(1.0_f64, f64::min);
        if min_kernel_weight_sum < 1.0 - 1e-10 {
            return Err(format!("marker Peskin kernel clipped by no-slip wall: min weight sum={min_kernel_weight_sum:.9}"));
        }
        let kernel_prepare_ms = prepare_start.elapsed().as_secs_f64() * 1000.0;
        let coupled_start = Instant::now();
        let mut background = vec![0.0; g.len];
        for (i, b) in background.iter_mut().enumerate() {
            *b = background_force[usize::from(i >= g.n * g.n)];
        }
        let mut fluid_calls = 0;
        let mut pressure_iterations = 0;
        let lagged_velocity = if let Some(p) = prior {
            &p.grid_velocity
        } else {
            // This allocation is only used for the first call in a trajectory.
            // Keep it alive until interpolation below.
            let (u, _, c) = solve_stokes(
                g,
                &mut self.backend,
                &background,
                None,
                self.profiling,
                &mut self.profile,
            )?;
            fluid_calls += 1;
            pressure_iterations += c.outer_iters;
            // Avoid a lifetime tied to the temporary branch.
            return self.solve_lagged_with_sample(
                markers,
                prepared,
                background,
                &u,
                None,
                None,
                omega,
                min_kernel_weight_sum,
                kernel_prepare_ms,
                coupled_start,
                fluid_calls,
                pressure_iterations,
            );
        };
        self.solve_lagged_with_sample(
            markers,
            prepared,
            background,
            lagged_velocity,
            prior.map(|p| p.marker_forces.as_slice()),
            prior.map(|p| p.pressure.as_slice()),
            omega,
            min_kernel_weight_sum,
            kernel_prepare_ms,
            coupled_start,
            fluid_calls,
            pressure_iterations,
        )
    }

    fn solve_lagged_with_sample(
        &mut self,
        markers: &[Marker],
        prepared: Vec<PreparedMarker>,
        background: Vec<f64>,
        lagged_velocity: &[f64],
        prior_forces: Option<&[[f64; 2]]>,
        prior_pressure: Option<&[f64]>,
        omega: f64,
        min_kernel_weight_sum: f64,
        kernel_prepare_ms: f64,
        coupled_start: Instant,
        mut fluid_calls: usize,
        mut pressure_iterations: usize,
    ) -> Result<Solution, String> {
        let g = &self.grid;
        let lagged_sample = sample(&prepared, &lagged_velocity);
        let point_update_start = Instant::now();
        let mut force = vec![0.0; markers.len() * 2];
        for (i, m) in markers.iter().enumerate() {
            let trial_x = m.drag * (m.velocity[0] - lagged_sample[2 * i]);
            let trial_y = m.drag * (m.velocity[1] - lagged_sample[2 * i + 1]);
            if omega == 1.0 {
                force[2 * i] = trial_x;
                force[2 * i + 1] = trial_y;
            } else {
                let old = prior_forces.map_or([0.0, 0.0], |p| p[i]);
                force[2 * i] = (1.0 - omega) * old[0] + omega * trial_x;
                force[2 * i + 1] = (1.0 - omega) * old[1] + omega * trial_y;
            }
        }
        let point_force_update_ms = point_update_start.elapsed().as_secs_f64() * 1000.0;
        let marker_grid_force = spread(g, &prepared, &force);
        let full_force: Vec<f64> = background
            .iter()
            .zip(&marker_grid_force)
            .map(|(a, b)| a + b)
            .collect();
        let (grid_velocity, pressure, count) = solve_stokes(
            g,
            &mut self.backend,
            &full_force,
            if cfg!(feature = "warm-pressure") {
                prior_pressure.map(|p| (lagged_velocity, p))
            } else {
                None
            },
            self.profiling,
            &mut self.profile,
        )?;
        fluid_calls += 1;
        pressure_iterations += count.outer_iters;
        let coupled_solve_ms = coupled_start.elapsed().as_secs_f64() * 1000.0;
        let diagnostics_start = Instant::now();
        let sampled = sample(&prepared, &grid_velocity);
        let mut closure = vec![0.0; force.len()];
        for (i, m) in markers.iter().enumerate() {
            closure[2 * i] = force[2 * i] / m.drag + sampled[2 * i] - m.velocity[0];
            closure[2 * i + 1] = force[2 * i + 1] / m.drag + sampled[2 * i + 1] - m.velocity[1];
        }
        let force_law_rms = if force.is_empty() {
            0.0
        } else {
            norm(&closure) / (force.len() as f64).sqrt()
        };
        let force_law_max_component_abs = closure.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
        let exchange_power: f64 = markers
            .iter()
            .enumerate()
            .map(|(i, m)| {
                force[2 * i] * (sampled[2 * i] - m.velocity[0])
                    + force[2 * i + 1] * (sampled[2 * i + 1] - m.velocity[1])
            })
            .sum();
        let mut divergence = vec![0.0; g.n * g.n];
        g.divergence(&grid_velocity, &mut divergence);
        let divergence_rms = norm(&divergence) / (divergence.len() as f64).sqrt();
        let mut au = vec![0.0; g.len];
        let mut gp = vec![0.0; g.len];
        g.apply_a(&grid_velocity, &mut au);
        g.gradient(&pressure, &mut gp);
        let momentum_rms = au
            .iter()
            .zip(&gp)
            .zip(&full_force)
            .map(|((a, p), f)| (a + p - f).powi(2))
            .sum::<f64>()
            .sqrt()
            / (g.len as f64).sqrt();
        let marker_sum = [
            force.iter().step_by(2).sum::<f64>(),
            force.iter().skip(1).step_by(2).sum::<f64>(),
        ];
        let grid_sum = [
            marker_grid_force[..g.n * g.n].iter().sum::<f64>() * g.h * g.h,
            marker_grid_force[g.n * g.n..].iter().sum::<f64>() * g.h * g.h,
        ];
        let grid_force_balance_abs =
            ((marker_sum[0] - grid_sum[0]).powi(2) + (marker_sum[1] - grid_sum[1]).powi(2)).sqrt();
        let diagnostics_ms = diagnostics_start.elapsed().as_secs_f64() * 1000.0;
        Ok(Solution {
            grid_velocity,
            pressure,
            marker_forces: force.chunks_exact(2).map(|f| [f[0], f[1]]).collect(),
            sampled_velocity: sampled.chunks_exact(2).map(|u| [u[0], u[1]]).collect(),
            coupling_iterations: 0,
            fluid_calls,
            pressure_iterations,
            divergence_rms,
            momentum_rms,
            force_law_rms,
            force_law_max_component_abs,
            exchange_power,
            point_force_update_ms,
            grid_force_balance_abs,
            min_kernel_weight_sum,
            kernel_prepare_ms,
            coupled_solve_ms,
            diagnostics_ms,
        })
    }
}
