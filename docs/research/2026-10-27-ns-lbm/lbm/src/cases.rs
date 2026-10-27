use crate::{
    gpu::{Context, Gpu, Result},
    model::{Config, Cpu, Field, initialize},
};
use serde_json::{Value, json};
use std::{
    f64::consts::{PI, TAU},
    io::Write,
    path::Path,
};
pub fn channel(y: f64, h: f64, nu: f64, g: f64) -> f64 {
    g * y * (h - y) / (2. * nu)
}
pub fn pulse(y: f64, t: f64, h: f64, nu: f64, g0: f64, g1: f64) -> f64 {
    let mut u = channel(y, h, nu, g0);
    for odd in (1..=255).step_by(2) {
        let m = odd as f64;
        let k = m * PI / h;
        let l = nu * k * k;
        u += 4. * g1 / (m * PI) * (k * y).sin() * (l * (TAU * t).cos() + TAU * (TAU * t).sin())
            / (l * l + TAU * TAU);
    }
    u
}
pub fn vortex(x: f64, y: f64, t: f64) -> [f64; 2] {
    let a = (-0.1 * t).exp();
    let x = x - 2. * t;
    let y = y - 0.7 * t;
    [2. + a * x.sin() * y.cos(), 0.7 - a * x.cos() * y.sin()]
}
fn save_profile(
    out: &Path,
    name: &str,
    c: Config,
    f: &Field,
    exact: impl Fn(f64, f64) -> [f64; 2],
) -> Result<()> {
    let mut w = std::io::BufWriter::new(std::fs::File::create(
        out.join(format!("{name}-field.csv")),
    )?);
    writeln!(w, "x,y,rho,u,v,exact_u,exact_v")?;
    for (i, p) in f.samples.iter().enumerate() {
        let [x, y] = c.position(i);
        let u = exact(x, y);
        writeln!(w, "{x},{y},{},{},{},{},{}", p[0], p[1], p[2], u[0], u[1])?;
    }
    Ok(())
}
pub fn compare(ctx: &Context) -> Result<Value> {
    let mut rows = Vec::new();
    for periodic in [true, false] {
        let c = Config::new(24, 12, 6., 3., 0.1, periodic, 2., 1);
        let a = [0.03, 0.];
        let u = |x: f64, y: f64| [0.1 * (TAU * y / 3.).sin(), 0.02 * (TAU * x / 6.).sin()];
        let mut cpu = Cpu::new(c, u, |_, _| 1., a);
        let mut gpu = Gpu::new(ctx, c, &cpu.f)?;
        let mut kicks = vec![[0_f64; 2]; c.len()];
        kicks[110] = [0.001, -0.0004];
        cpu.kick(&kicks);
        let k32: Vec<[f32; 2]> = kicks.iter().map(|x| [x[0] as f32, x[1] as f32]).collect();
        let mut field = cpu.field(a);
        for block in 0..30 {
            for _ in 0..c.substeps {
                cpu.step(a);
            }
            field = gpu
                .advance(
                    &vec![a; c.substeps],
                    a,
                    if block == 0 { Some(&k32) } else { None },
                )?
                .0;
        }
        let reference = cpu.field(a);
        let (mut err, mut norm, mut max_abs) = (0., 0., 0_f64);
        for (p, r) in field.samples.iter().zip(&reference.samples) {
            for k in 1..3 {
                err += (p[k] - r[k]).powi(2);
                norm += r[k].powi(2);
                max_abs = max_abs.max((p[k] - r[k]).abs());
            }
        }
        let relative = (err / norm).sqrt();
        assert!(
            relative < 2e-4 && max_abs < 2e-5,
            "GPU/f64 mismatch {relative} {max_abs}"
        );
        rows.push(json!({"periodic_y":periodic,"config":c.metadata(),"steps":30*c.substeps,"relative_velocity_l2":relative,"max_abs_velocity":max_abs,"diagnostics":field.diagnostics(c)}));
    }
    Ok(json!({"adapter":ctx.info,"cases":rows}))
}
pub fn physics_case(
    ctx: &Context,
    kind: &str,
    n: usize,
    re: f64,
    refine: usize,
    out: &Path,
) -> Result<Value> {
    std::fs::create_dir_all(out)?;
    let c = if kind == "vortex" {
        Config::new(n, n, TAU, TAU, 0.05, true, 2_f64.hypot(0.7) + 1., refine)
    } else {
        Config::new(n, n * 5 / 32, 48., 7.5, 75. / re, false, 20., refine)
    };
    let g0 = 12. * c.nu * 10. / (c.ly * c.ly);
    let g1 = 0.8 * g0;
    let force = |t: f64| {
        if kind == "vortex" {
            [0., 0.]
        } else if kind == "pulse" {
            [g0 + g1 * (TAU * t).cos(), 0.]
        } else {
            [g0, 0.]
        }
    };
    let exact = |x: f64, y: f64, t: f64| {
        if kind == "vortex" {
            vortex(x, y, t)
        } else if kind == "pulse" {
            [pulse(y, t, c.ly, c.nu, g0, g1), 0.]
        } else {
            [channel(y, c.ly, c.nu, g0), 0.]
        }
    };
    let rho = |x: f64, y: f64| {
        if kind == "vortex" {
            1. + 0.75 * (c.dt / c.h).powi(2) * ((2. * x).cos() + (2. * y).cos())
        } else {
            1.
        }
    };
    let initial = initialize(c, |x, y| exact(x, y, 0.), rho, force(0.));
    let mut gpu = Gpu::new(ctx, c, &initial)?;
    let name = format!("{kind}-n{n}-re{re}-r{refine}");
    let mut trace = std::io::BufWriter::new(std::fs::File::create(
        out.join(format!("{name}-trace.csv")),
    )?);
    writeln!(
        trace,
        "t,relative_l2,mean_u,exact_mean_u,rho_min,rho_max,max_mach,gpu_ms,whole_ms"
    )?;
    let blocks = if kind == "steady" { 240 } else { 120 };
    let translation = if kind == "vortex" {
        [2., 0.7]
    } else {
        [0., 0.]
    };
    let (mut max_error, mut total_ms, mut gpu_ms) = (0_f64, 0., 0.);
    let mut last = None;
    for b in 0..blocks {
        let a: Vec<_> = (0..c.substeps)
            .map(|k| force((b * c.substeps + k) as f64 * c.dt))
            .collect();
        let t = (b + 1) as f64 / 120.;
        let (field, times) = gpu.advance(&a, force(t), None)?;
        let error = field.relative_error(c, |x, y| exact(x, y, t), translation);
        max_error = max_error.max(error);
        let d = field.diagnostics(c);
        let mean = (0..c.len())
            .map(|i| {
                let [x, y] = c.position(i);
                exact(x, y, t)[0]
            })
            .sum::<f64>()
            / c.len() as f64;
        writeln!(
            trace,
            "{t},{error},{},{mean},{},{},{},{},{}",
            d["mean_u"], d["rho_min"], d["rho_max"], d["max_mach"], times.gpu, times.whole
        )?;
        total_ms += times.whole;
        gpu_ms += times.gpu;
        last = Some(field);
    }
    let field = last.unwrap();
    let t = blocks as f64 / 120.;
    let error = field.relative_error(c, |x, y| exact(x, y, t), translation);
    save_profile(out, &name, c, &field, |x, y| exact(x, y, t))?;
    let row = json!({"case":kind,"config":c.metadata(),"re_reference":re,"refinement":refine,"seconds":t,"steps":blocks*c.substeps,"relative_l2":error,"max_relative_l2":max_error,"diagnostics":field.diagnostics(c),"advance_total_ms":total_ms,"device_total_ms":gpu_ms,"adapter":ctx.info});
    std::fs::write(
        out.join(format!("{name}.json")),
        serde_json::to_string_pretty(&row)?,
    )?;
    Ok(row)
}
