use crate::grid::{Config, Grid};
use serde_json::{Value, json};
use std::f64::consts::{PI, TAU};
use std::io::Write;
use std::path::Path;

pub fn channel(y: f64, h: f64, nu: f64, g: f64) -> f64 {
    g * y * (h - y) / (2.0 * nu)
}
/// Independent continuum sine-series solution for uniform harmonic acceleration.
/// Re[e^(i omega t)/(lambda+i omega)] =
/// (lambda cos(omega t)+omega sin(omega t))/(lambda^2+omega^2).
pub fn pulse(y: f64, t: f64, h: f64, nu: f64, g0: f64, g1: f64, omega: f64) -> f64 {
    let mut u = channel(y, h, nu, g0);
    for odd in (1..=255).step_by(2) {
        let m = odd as f64;
        let k = m * PI / h;
        let lambda = nu * k * k;
        u += 4.0 * g1 / (m * PI)
            * (k * y).sin()
            * (lambda * (omega * t).cos() + omega * (omega * t).sin())
            / (lambda * lambda + omega * omega);
    }
    u
}
fn vortex(x: f64, y: f64, t: f64, nu: f64) -> [f64; 2] {
    let a = (-2.0 * nu * t).exp();
    let x = x - 2.0 * t;
    let y = y - 0.7 * t;
    [2.0 + a * x.sin() * y.cos(), 0.7 - a * x.cos() * y.sin()]
}
fn fluctuation_error(g: &Grid, t: f64) -> f64 {
    let mut err = 0.0;
    let mut norm = 0.0;
    for (component, field) in [(0, &g.u), (1, &g.v)] {
        for (idx, &u) in field.iter().enumerate() {
            let [x, y] = g.c.position(component, idx);
            let exact = vortex(x, y, t, g.c.nu)[component];
            err += (u - exact).powi(2);
            norm += (exact - [2.0, 0.7][component]).powi(2);
        }
    }
    (err / norm).sqrt()
}
fn profile(path: &Path, g: &Grid, exact: impl Fn(f64) -> f64) {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    writeln!(f, "y,u,exact").unwrap();
    for j in 0..g.c.ny {
        let y = (j as f64 + 0.5) * g.c.hy();
        writeln!(f, "{y:.12},{:.12},{:.12}", g.u[j * g.c.nx], exact(y)).unwrap();
    }
}
pub fn physics(out: &Path) -> Value {
    std::fs::create_dir_all(out).unwrap();
    let mut rows = Vec::new();
    for re in [0.1, 100.0, 300.0] {
        for (nx, ny) in [(192, 30), (256, 40)] {
            let h = 7.5;
            let nu = 10.0 * h / re;
            let force = 12.0 * nu * 10.0 / (h * h);
            let c = Config::new(nx, ny, 48.0, h, 1.0 / 120.0, nu, false);
            let mut g = Grid::new(c);
            g.set_velocity(|_, y| [channel(y, h, nu, force), 0.0]);
            for _ in 0..240 {
                g.step(force);
            }
            let e = g.relative_error(|_, y| [channel(y, h, nu, force), 0.0]);
            let d = g.diagnostics();
            profile(&out.join(format!("steady-re{re}-{nx}.csv")), &g, |y| {
                channel(y, h, nu, force)
            });
            rows.push(
                json!({"case":"steady","nx":nx,"ny":ny,"re":re,"nu":nu,"dt":c.dt,"seconds":2.0,
            "relative_l2":e,"divergence_rms":d.0,"mean_u":d.3,"max_component":d.2}),
            );
            eprintln!("steady Re={re} {nx}x{ny} error={e:.6}");
        }
    }
    for (nx, ny) in [(192, 30), (256, 40)] {
        for dt in [1.0 / 120.0, 1.0 / 240.0] {
            let h = 7.5;
            let nu = 0.75;
            let g0 = 12.0 * nu * 10.0 / (h * h);
            let g1 = 0.8 * g0;
            let omega = TAU;
            let mut g = Grid::new(Config::new(nx, ny, 48.0, h, dt, nu, false));
            g.set_velocity(|_, y| [pulse(y, 0.0, h, nu, g0, g1, omega), 0.0]);
            let mut trace = std::io::BufWriter::new(
                std::fs::File::create(out.join(format!("pulse-trace-{nx}-{dt:.6}.csv"))).unwrap(),
            );
            writeln!(trace, "t,mean_u,exact_mean_u,relative_l2").unwrap();
            let steps = (1.0 / dt).round() as usize;
            let mut max_error: f64 = 0.0;
            for k in 1..=steps {
                let t = k as f64 * dt;
                g.step(g0 + g1 * (omega * t).cos());
                let e = g.relative_error(|_, y| [pulse(y, t, h, nu, g0, g1, omega), 0.0]);
                max_error = max_error.max(e);
                let exact_mean = (0..ny)
                    .map(|j| pulse((j as f64 + 0.5) * g.c.hy(), t, h, nu, g0, g1, omega))
                    .sum::<f64>()
                    / ny as f64;
                writeln!(
                    trace,
                    "{t:.12},{:.12},{exact_mean:.12},{e:.12}",
                    g.diagnostics().3
                )
                .unwrap();
            }
            let e = g.relative_error(|_, y| [pulse(y, 1.0, h, nu, g0, g1, omega), 0.0]);
            profile(&out.join(format!("pulse-{nx}-{dt:.6}.csv")), &g, |y| {
                pulse(y, 1.0, h, nu, g0, g1, omega)
            });
            rows.push(json!({"case":"pulse","nx":nx,"ny":ny,"re":100.0,"nu":nu,"dt":dt,"seconds":1.0,
            "womersley_half_width":h*0.5*(omega/nu).sqrt(),"relative_l2":e,"max_relative_l2":max_error,"divergence_rms":g.diagnostics().0}));
            eprintln!("pulse {nx} dt={dt:.6} error={e:.6} max={max_error:.6}");
        }
    }
    for nx in [64, 128, 192, 256] {
        for dt in [1.0 / 120.0, 1.0 / 240.0] {
            let nu = 0.05;
            let t = 1.0;
            let mut g = Grid::new(Config::new(nx, nx, TAU, TAU, dt, nu, true));
            g.set_velocity(|x, y| vortex(x, y, 0.0, nu));
            for _ in 0..(t / dt).round() as usize {
                g.step(0.0);
            }
            let e = fluctuation_error(&g, t);
            rows.push(
                json!({"case":"translated_taylor_green","nx":nx,"ny":nx,"nu":nu,"dt":dt,"seconds":t,
            "fluctuation_relative_l2":e,"divergence_rms":g.diagnostics().0,
            "max_translation_courant":2.0*dt/g.c.hx()}),
            );
            eprintln!("vortex {nx} dt={dt:.6} perturbation error={e:.6}");
        }
    }
    json!({"cases":rows,"pulse_series_max_odd":255})
}
