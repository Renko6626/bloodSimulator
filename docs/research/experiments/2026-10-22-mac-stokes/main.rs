use std::env;
use std::f64::consts::PI;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

mod separable;
use separable::SeparableInverse;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scene {
    Poiseuille,
    Manufactured,
    Localized,
}

impl Scene {
    fn name(self) -> &'static str {
        match self {
            Self::Poiseuille => "poiseuille",
            Self::Manufactured => "manufactured",
            Self::Localized => "localized",
        }
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

fn exact(scene: Scene, x: f64, y: f64, phase: f64) -> (f64, f64, f64, f64, f64) {
    if scene == Scene::Poiseuille {
        return (0.5 * y * (1.0 - y), 0.0, 0.0, 1.0, 0.0);
    }
    if scene == Scene::Localized {
        let gauss = |cx: f64, cy: f64| -> f64 {
            let dx = (x - cx).abs().min(1.0 - (x - cx).abs());
            let dy = y - cy;
            (-(dx * dx + dy * dy) / (2.0 * 0.05_f64.powi(2))).exp()
        };
        let fx = gauss(0.30, 0.42) - 0.7 * gauss(0.72, 0.62);
        let fy = 0.8 * gauss(0.53, 0.30);
        return (f64::NAN, f64::NAN, f64::NAN, fx, fy);
    }
    let k = 2.0 * PI;
    let sx = (k * x + phase).sin();
    let cx = (k * x + phase).cos();
    let sy = (PI * y).sin();
    let cy = (PI * y).cos();
    let c2y = (2.0 * PI * y).cos();
    let u = PI * sx * (2.0 * PI * y).sin();
    let v = -k * cx * sy * sy;
    let p = cx * cy;
    let lap_u = -(k * k + 4.0 * PI * PI) * u;
    let lap_v = -k * k * v - 2.0 * k * PI * PI * cx * c2y;
    let fx = -lap_u - k * sx * cy;
    let fy = -lap_v - PI * cx * sy;
    (u, v, p, fx, fy)
}

fn make_rhs(g: &Grid, scene: Scene, phase: f64) -> Vec<f64> {
    let mut f = vec![0.0; g.len];
    for j in 0..g.n {
        for i in 0..g.n {
            f[g.u(i, j)] = exact(scene, i as f64 * g.h, (j as f64 + 0.5) * g.h, phase).3;
        }
    }
    for j in 1..g.n {
        for i in 0..g.n {
            f[g.v(i, j)] = exact(scene, (i as f64 + 0.5) * g.h, j as f64 * g.h, phase).4;
        }
    }
    f
}

struct Metrics {
    u_l2: f64,
    p_l2: f64,
    div_rms: f64,
    momentum_rms: f64,
    mass_flux: f64,
}
fn metrics(g: &Grid, scene: Scene, phase: f64, f: &[f64], u: &[f64], p: &[f64]) -> Metrics {
    let mut vel_e2 = 0.0;
    let mut pressure_e2 = 0.0;
    let mut exact_p = vec![0.0; p.len()];
    if scene != Scene::Localized {
        for j in 0..g.n {
            for i in 0..g.n {
                let idx = g.u(i, j);
                let val = exact(scene, i as f64 * g.h, (j as f64 + 0.5) * g.h, phase).0;
                vel_e2 += (u[idx] - val).powi(2);
                exact_p[g.p(i, j)] =
                    exact(scene, (i as f64 + 0.5) * g.h, (j as f64 + 0.5) * g.h, phase).2;
            }
        }
        for j in 1..g.n {
            for i in 0..g.n {
                let idx = g.v(i, j);
                let val = exact(scene, (i as f64 + 0.5) * g.h, j as f64 * g.h, phase).1;
                vel_e2 += (u[idx] - val).powi(2);
            }
        }
        zero_mean(&mut exact_p);
        for k in 0..p.len() {
            pressure_e2 += (p[k] - exact_p[k]).powi(2);
        }
    }
    let mut div = vec![0.0; p.len()];
    g.divergence(u, &mut div);
    let mut au = vec![0.0; g.len];
    let mut gp = vec![0.0; g.len];
    g.apply_a(u, &mut au);
    g.gradient(p, &mut gp);
    let residual: Vec<f64> = au
        .iter()
        .zip(&gp)
        .zip(f)
        .map(|((a, b), fi)| a + b - fi)
        .collect();
    let flux = (0..g.n).map(|j| u[g.u(0, j)] * g.h).sum();
    Metrics {
        u_l2: if scene == Scene::Localized {
            f64::NAN
        } else {
            (vel_e2 / g.len as f64).sqrt()
        },
        p_l2: if scene == Scene::Localized {
            f64::NAN
        } else {
            (pressure_e2 / p.len() as f64).sqrt()
        },
        div_rms: (dot(&div, &div) / div.len() as f64).sqrt(),
        momentum_rms: (dot(&residual, &residual) / residual.len() as f64).sqrt(),
        mass_flux: flux,
    }
}

fn percentile(v: &[f64], q: f64) -> f64 {
    let mut w = v.to_vec();
    w.sort_by(|a, b| a.total_cmp(b));
    w[((w.len() - 1) as f64 * q).ceil() as usize]
}
fn json_num(x: f64) -> String {
    if x.is_finite() {
        x.to_string()
    } else {
        "null".to_string()
    }
}
fn parse_args() -> Result<
    (
        Vec<Scene>,
        Vec<usize>,
        usize,
        String,
        Option<String>,
        String,
        Option<String>,
    ),
    String,
> {
    let mut scenes = vec![Scene::Poiseuille, Scene::Manufactured, Scene::Localized];
    let mut sizes = vec![32, 64, 128];
    let mut runs = 1;
    let mut format = "csv".to_string();
    let mut export_dir = None;
    let mut backend = "cg".to_string();
    let mut samples_out = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        let val = args
            .next()
            .ok_or_else(|| format!("missing value for {arg}"))?;
        match arg.as_str() {
            "--scene" => {
                scenes = match val.as_str() {
                    "all" => vec![Scene::Poiseuille, Scene::Manufactured, Scene::Localized],
                    "poiseuille" => vec![Scene::Poiseuille],
                    "manufactured" => vec![Scene::Manufactured],
                    "localized" => vec![Scene::Localized],
                    _ => {
                        return Err(
                            "scene must be all, poiseuille, manufactured or localized".into()
                        )
                    }
                }
            }
            "--sizes" => {
                sizes = val
                    .split(',')
                    .map(|s| s.parse::<usize>().map_err(|_| "invalid size".to_string()))
                    .collect::<Result<Vec<_>, _>>()?;
                if sizes.is_empty() || sizes.iter().any(|&n| n < 4) {
                    return Err("sizes must be >=4".into());
                }
            }
            "--runs" => {
                runs = val.parse().map_err(|_| "invalid runs".to_string())?;
                if runs == 0 {
                    return Err("runs must be >=1".into());
                }
            }
            "--format" => {
                if val != "csv" && val != "json" {
                    return Err("format must be csv or json".into());
                }
                format = val
            }
            "--export-dir" => export_dir = Some(val),
            "--backend" => {
                if val != "cg" && val != "separable" {
                    return Err("backend must be cg or separable".into());
                }
                backend = val
            }
            "--samples-out" => samples_out = Some(val),
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    Ok((
        scenes,
        sizes,
        runs,
        format,
        export_dir,
        backend,
        samples_out,
    ))
}

fn write_fields(dir: &str, g: &Grid, scene: Scene, u: &[f64], p: &[f64]) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let stem = format!("{}_{}", scene.name(), g.n);
    let mut cells = BufWriter::new(
        File::create(Path::new(dir).join(format!("{}_cells.csv", stem)))
            .map_err(|e| e.to_string())?,
    );
    let mut uf = BufWriter::new(
        File::create(Path::new(dir).join(format!("{}_u_faces.csv", stem)))
            .map_err(|e| e.to_string())?,
    );
    let mut vf = BufWriter::new(
        File::create(Path::new(dir).join(format!("{}_v_faces.csv", stem)))
            .map_err(|e| e.to_string())?,
    );
    writeln!(cells, "x,y,ux_center,uy_center,p").map_err(|e| e.to_string())?;
    writeln!(uf, "x,y,u").map_err(|e| e.to_string())?;
    writeln!(vf, "x,y,v").map_err(|e| e.to_string())?;
    for j in 0..g.n {
        for i in 0..g.n {
            let x = (i as f64 + 0.5) * g.h;
            let y = (j as f64 + 0.5) * g.h;
            let uc = 0.5 * (u[g.u(i, j)] + u[g.u((i + 1) % g.n, j)]);
            let vb = if j == 0 { 0.0 } else { u[g.v(i, j)] };
            let vt = if j + 1 == g.n { 0.0 } else { u[g.v(i, j + 1)] };
            writeln!(
                cells,
                "{x:.12e},{y:.12e},{uc:.12e},{:.12e},{:.12e}",
                0.5 * (vb + vt),
                p[g.p(i, j)]
            )
            .map_err(|e| e.to_string())?;
            writeln!(uf, "{:.12e},{y:.12e},{:.12e}", i as f64 * g.h, u[g.u(i, j)])
                .map_err(|e| e.to_string())?;
        }
    }
    for j in 0..=g.n {
        for i in 0..g.n {
            let x = (i as f64 + 0.5) * g.h;
            let y = j as f64 * g.h;
            let v = if j == 0 || j == g.n {
                0.0
            } else {
                u[g.v(i, j)]
            };
            writeln!(vf, "{x:.12e},{y:.12e},{v:.12e}").map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let (scenes, sizes, runs, format, export_dir, backend_name, samples_out) = parse_args()?;
    let mut sample_writer = if let Some(path) = samples_out {
        let mut w = BufWriter::new(File::create(path).map_err(|e| e.to_string())?);
        writeln!(
            w,
            "backend,scene,n,rep,cold_or_warm,elapsed_ms,outer_iters,inner_iters,inverse_calls"
        )
        .map_err(|e| e.to_string())?;
        Some(w)
    } else {
        None
    };
    if format == "csv" {
        println!("backend,scene,n,runs,warm_runs,backend_init_ms,init_ms_mean,cold_ms_mean,cold_ms_p95,warm_ms_mean,warm_ms_p95,cold_outer_mean,cold_inner_mean,cold_inverse_calls_mean,warm_outer_mean,warm_inner_mean,warm_inverse_calls_mean,warm_u_diff_max,warm_p_diff_max,warm_div_rms_max,warm_momentum_rms_max,u_l2,p_l2,div_rms,momentum_rms,mass_flux");
    } else {
        println!("[");
    }
    let mut first = true;
    for scene in scenes {
        for &n in &sizes {
            let t = Instant::now();
            let backend = Backend::new(&backend_name, n)?;
            let backend_init_ms = t.elapsed().as_secs_f64() * 1000.0;
            let mut init = Vec::new();
            let mut cold = Vec::new();
            let mut warm = Vec::new();
            let mut cold_outer = 0usize;
            let mut cold_inner = 0usize;
            let mut cold_inverse = 0usize;
            let mut warm_outer = 0usize;
            let mut warm_inner = 0usize;
            let mut warm_inverse = 0usize;
            let mut last_metrics = None;
            let mut previous: Option<(Vec<f64>, Vec<f64>)> = None;
            let mut last_cold = None;
            let mut warm_u_diff_max = 0.0_f64;
            let mut warm_p_diff_max = 0.0_f64;
            let mut warm_div_max = 0.0_f64;
            let mut warm_momentum_max = 0.0_f64;
            for rep in 0..runs {
                let t = Instant::now();
                let g = Grid::new(n);
                let phase = if scene == Scene::Manufactured {
                    0.31 * rep as f64
                } else {
                    0.0
                };
                let f = make_rhs(&g, scene, phase);
                init.push(t.elapsed().as_secs_f64() * 1000.0);
                let t = Instant::now();
                let (u, p, c) = solve_stokes(&g, &backend, &f, None)?;
                let elapsed = t.elapsed().as_secs_f64() * 1000.0;
                cold.push(elapsed);
                cold_outer += c.outer_iters;
                cold_inner += c.inner_iters;
                cold_inverse += c.inverse_calls;
                if let Some(w) = sample_writer.as_mut() {
                    writeln!(
                        w,
                        "{},{},{},{},cold,{:.9},{},{},{}",
                        backend_name,
                        scene.name(),
                        n,
                        rep,
                        elapsed,
                        c.outer_iters,
                        c.inner_iters,
                        c.inverse_calls
                    )
                    .map_err(|e| e.to_string())?;
                }
                last_metrics = Some(metrics(&g, scene, phase, &f, &u, &p));
                if export_dir.is_some() {
                    last_cold = Some((u.clone(), p.clone()));
                }
                // Change the x phase of the analytic forcing; the previous solution is a genuine initial guess.
                if scene == Scene::Manufactured && runs > 1 && rep > 0 {
                    let (ref pu, ref pp) = previous.as_ref().unwrap();
                    let t = Instant::now();
                    let (wu, wp, wc) = solve_stokes(&g, &backend, &f, Some((pu, pp)))?;
                    let elapsed = t.elapsed().as_secs_f64() * 1000.0;
                    warm.push(elapsed);
                    warm_outer += wc.outer_iters;
                    warm_inner += wc.inner_iters;
                    warm_inverse += wc.inverse_calls;
                    if let Some(w) = sample_writer.as_mut() {
                        writeln!(
                            w,
                            "{},{},{},{},warm,{:.9},{},{},{}",
                            backend_name,
                            scene.name(),
                            n,
                            rep,
                            elapsed,
                            wc.outer_iters,
                            wc.inner_iters,
                            wc.inverse_calls
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    let wm = metrics(&g, scene, phase, &f, &wu, &wp);
                    warm_div_max = warm_div_max.max(wm.div_rms);
                    warm_momentum_max = warm_momentum_max.max(wm.momentum_rms);
                    warm_u_diff_max = warm_u_diff_max.max(
                        wu.iter()
                            .zip(&u)
                            .map(|(a, b)| (a - b).abs())
                            .fold(0.0_f64, f64::max),
                    );
                    warm_p_diff_max = warm_p_diff_max.max(
                        wp.iter()
                            .zip(&p)
                            .map(|(a, b)| (a - b).abs())
                            .fold(0.0_f64, f64::max),
                    );
                    previous = Some((wu, wp));
                } else {
                    previous = Some((u, p));
                }
            }
            let m = last_metrics.unwrap();
            if let (Some(dir), Some((u, p))) = (&export_dir, last_cold) {
                write_fields(dir, &Grid::new(n), scene, &u, &p)?;
            }
            let mean = |v: &Vec<f64>| -> f64 { v.iter().sum::<f64>() / v.len() as f64 };
            let wm = if warm.is_empty() {
                f64::NAN
            } else {
                mean(&warm)
            };
            let wp = if warm.is_empty() {
                f64::NAN
            } else {
                percentile(&warm, 0.95)
            };
            let wo = if warm.is_empty() {
                f64::NAN
            } else {
                warm_outer as f64 / warm.len() as f64
            };
            let wi = if warm.is_empty() {
                f64::NAN
            } else {
                warm_inner as f64 / warm.len() as f64
            };
            let wc = if warm.is_empty() {
                f64::NAN
            } else {
                warm_inverse as f64 / warm.len() as f64
            };
            if format == "csv" {
                println!("{},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e},{:.9e}",backend_name,scene.name(),n,runs,warm.len(),backend_init_ms,mean(&init),mean(&cold),percentile(&cold,0.95),wm,wp,cold_outer as f64/runs as f64,cold_inner as f64/runs as f64,cold_inverse as f64/runs as f64,wo,wi,wc,if warm.is_empty(){f64::NAN}else{warm_u_diff_max},if warm.is_empty(){f64::NAN}else{warm_p_diff_max},if warm.is_empty(){f64::NAN}else{warm_div_max},if warm.is_empty(){f64::NAN}else{warm_momentum_max},m.u_l2,m.p_l2,m.div_rms,m.momentum_rms,m.mass_flux);
            } else {
                if !first {
                    println!(",")
                }
                first = false;
                let mut fields = Vec::new();
                fields.push(format!("\"backend\":\"{}\"", backend_name));
                fields.push(format!("\"scene\":\"{}\"", scene.name()));
                fields.push(format!(
                    "\"n\":{n},\"runs\":{runs},\"warm_runs\":{}",
                    warm.len()
                ));
                fields.push(format!(
                    "\"backend_init_ms\":{backend_init_ms},\"init_ms_mean\":{}",
                    mean(&init)
                ));
                fields.push(format!(
                    "\"cold_ms_mean\":{},\"cold_ms_p95\":{}",
                    mean(&cold),
                    percentile(&cold, 0.95)
                ));
                fields.push(format!(
                    "\"warm_ms_mean\":{},\"warm_ms_p95\":{}",
                    json_num(wm),
                    json_num(wp)
                ));
                fields.push(format!(
                    "\"cold_outer_mean\":{},\"cold_inner_mean\":{},\"cold_inverse_calls_mean\":{}",
                    cold_outer as f64 / runs as f64,
                    cold_inner as f64 / runs as f64,
                    cold_inverse as f64 / runs as f64
                ));
                fields.push(format!(
                    "\"warm_outer_mean\":{},\"warm_inner_mean\":{},\"warm_inverse_calls_mean\":{}",
                    json_num(wo),
                    json_num(wi),
                    json_num(wc)
                ));
                fields.push(format!("\"warm_u_diff_max\":{},\"warm_p_diff_max\":{},\"warm_div_rms_max\":{},\"warm_momentum_rms_max\":{}",json_num(if warm.is_empty(){f64::NAN}else{warm_u_diff_max}),json_num(if warm.is_empty(){f64::NAN}else{warm_p_diff_max}),json_num(if warm.is_empty(){f64::NAN}else{warm_div_max}),json_num(if warm.is_empty(){f64::NAN}else{warm_momentum_max})));
                fields.push(format!(
                    "\"u_l2\":{},\"p_l2\":{},\"div_rms\":{},\"momentum_rms\":{},\"mass_flux\":{}",
                    json_num(m.u_l2),
                    json_num(m.p_l2),
                    m.div_rms,
                    m.momentum_rms,
                    m.mass_flux
                ));
                print!("{{{}}}", fields.join(","));
            }
        }
    }
    if format == "json" {
        println!("\n]");
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1)
    }
}
