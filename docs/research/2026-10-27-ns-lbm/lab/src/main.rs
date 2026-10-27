mod cases;
mod coupling;
mod factor;
mod grid;
mod profile;
#[cfg(test)]
mod tests;
use coupling::Cells;
use grid::{Config, Grid, Times};
use serde_json::json;
use std::io::Write;
use std::path::Path;
use std::time::Instant;

fn bench(nx: usize, ny: usize, count: usize, frames: usize, prefix: &Path) {
    assert!(count == 0 || count == 72);
    let lx = 48.0;
    let ly = if nx == ny { 48.0 } else { 7.5 };
    let re = 100.0;
    let mean = 10.0;
    let nu = mean * ly / re;
    let force = 12.0 * nu * mean / (ly * ly);
    let c = Config::new(nx, ny, lx, ly, 1.0 / 120.0, nu, false);
    let init = Instant::now();
    let mut g = Grid::new(c);
    // Divergence-free channel perturbation from psi = A sin(kx x) sin^2(pi y/H).
    let a = 0.3;
    let kx = std::f64::consts::TAU / lx;
    let ky = std::f64::consts::PI / ly;
    g.set_velocity(|x, y| {
        [
            cases::channel(y, ly, nu, force)
                + a * 2.0 * ky * (kx * x).sin() * (ky * y).sin() * (ky * y).cos(),
            -a * kx * (kx * x).cos() * (ky * y).sin().powi(2),
        ]
    });
    g.project();
    let fluid_init_ms = init.elapsed().as_secs_f64() * 1e3;
    let t = Instant::now();
    let mut cells = if count > 0 {
        Some(Cells::new(&g, count))
    } else {
        None
    };
    let cell_init_ms = t.elapsed().as_secs_f64() * 1e3;
    let mut lines = String::from(
        "frame,t,advection_ms,diffusion_ms,pressure_ms,coupling_ms,rapier_ms,diagnostics_ms,fluid_coupling_ms,whole_ms,divergence_rms,energy,max_component,mean_u,max_area_rel,max_node_speed,cell_mean_vx,exchange_mismatch\n",
    );
    let warmup = 30;
    for frame in 0..warmup + frames {
        profile::frame(if frame >= warmup {
            Some(frame - warmup)
        } else {
            None
        });
        let whole = Instant::now();
        let mut times = Times::default();
        let mut coupling_ms = 0.0;
        let mut rapier_ms = 0.0;
        let mut mismatch: f64 = 0.0;
        for _ in 0..2 {
            if let Some(cells) = cells.as_mut() {
                let t = Instant::now();
                {
                    let _span = profile::span("coupling", 8);
                    mismatch = mismatch.max(cells.exchange(&mut g));
                }
                coupling_ms += t.elapsed().as_secs_f64() * 1e3;
            }
            times += g.step(force);
            if let Some(cells) = cells.as_mut() {
                let t = Instant::now();
                {
                    let _span = profile::span("rapier", 9);
                    cells.step(lx);
                }
                rapier_ms += t.elapsed().as_secs_f64() * 1e3;
            }
        }
        let t = Instant::now();
        let diagnostic_span = profile::span("diagnostics", 10);
        let d = g.diagnostics();
        assert!(d.0.is_finite() && d.1.is_finite() && d.2.is_finite());
        let cd = cells.as_ref().map_or((0.0, 0.0, 0.0), Cells::diagnostics);
        drop(diagnostic_span);
        let diagnostics_ms = t.elapsed().as_secs_f64() * 1e3;
        let whole_ms = whole.elapsed().as_secs_f64() * 1e3;
        let fluid = times.advection + times.diffusion + times.pressure + coupling_ms;
        // Serialization and file I/O are outside measured whole_ms.
        if frame >= warmup {
            use std::fmt::Write;
            writeln!(lines,"{},{:.9},{:.9},{:.9},{:.9},{coupling_ms:.9},{rapier_ms:.9},{diagnostics_ms:.9},{fluid:.9},{whole_ms:.9},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{mismatch:.12}",
                frame-warmup,(frame+1) as f64/60.0,times.advection,times.diffusion,times.pressure,d.0,d.1,d.2,d.3,cd.0,cd.1,cd.2).unwrap();
        }
    }
    profile::frame(None);
    profile::write(prefix);
    std::fs::write(prefix.with_extension("csv"), lines).unwrap();
    let d = g.diagnostics();
    let meta = json!({"nx":nx,"ny":ny,"active_pressure_cells":nx*ny,"lx":lx,"ly":ly,"re_reference":re,"nu":nu,"mean_reference":mean,
        "density_2d":1.0,"cell_mass":1.0,"radius":0.7,"cells":count,"nodes_per_cell":12,"markers_per_cell":4,"gamma_per_cell":0.25,
        "substeps":2,"dt":c.dt,"frames":frames,"warmup_frames":warmup,"fluid_init_ms":fluid_init_ms,"cell_init_ms":cell_init_ms,
        "field_fingerprint":g.fingerprint(),"rayon_threads":rayon::current_num_threads(),"profiling":std::env::var_os("NS_PROFILE").is_some(),"numeric_factor_bytes":g.factor_bytes(),"final_divergence_rms":d.0,"final_mean_u":d.3,
        "max_rss_kib":std::fs::read_to_string("/proc/self/status").unwrap().lines().find(|x|x.starts_with("VmHWM:")).unwrap_or("")});
    std::fs::write(
        prefix.with_extension("json"),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .unwrap();
    let mut profile = std::io::BufWriter::new(
        std::fs::File::create(prefix.with_extension("profile.csv")).unwrap(),
    );
    writeln!(profile, "y,u_centerline_x,v_centerline_x").unwrap();
    for j in 0..ny {
        let y = (j as f64 + 0.5) * c.hy();
        let v = g.velocity([lx * 0.5, y]);
        writeln!(profile, "{y:.12},{:.12},{:.12}", v[0], v[1]).unwrap();
    }
    println!("{}", meta);
}
fn main() {
    rayon::ThreadPoolBuilder::new()
        .num_threads(
            std::env::var("NS_THREADS")
                .map_or(4, |s| s.parse().expect("NS_THREADS positive integer")),
        )
        .build_global()
        .unwrap();
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("physics") => {
            let path = Path::new(&args[2]);
            let result = cases::physics(path);
            std::fs::write(
                path.join("physics.json"),
                serde_json::to_string_pretty(&result).unwrap(),
            )
            .unwrap();
        }
        Some("bench") => bench(
            args[2].parse().unwrap(),
            args[3].parse().unwrap(),
            args[4].parse().unwrap(),
            args[5].parse().unwrap(),
            Path::new(&args[6]),
        ),
        _ => panic!("usage: ns-mac-lab physics OUT_DIR | bench NX NY CELLS FRAMES OUT_PREFIX"),
    }
}
