mod cases;
mod coupling;
mod gpu;
mod model;
#[cfg(test)]
mod tests;
use gpu::{Context, Gpu, Result, Times};
use model::{Config, initialize};
use serde_json::json;
use std::{io::Write, path::Path, time::Instant};
fn bench(
    ctx: &Context,
    nx: usize,
    ny: usize,
    count: usize,
    frames: usize,
    re: f64,
    prefix: &Path,
) -> Result<()> {
    assert!([0, 72].contains(&count));
    let ly = if nx == ny { 48. } else { 7.5 };
    let nu = 10. * ly / re;
    let c = Config::new(nx, ny, 48., ly, nu, false, 20., 1);
    let force = [12. * nu * 10. / (ly * ly), 0.];
    let init = Instant::now();
    let a = 0.3;
    let kx = std::f64::consts::TAU / 48.;
    let ky = std::f64::consts::PI / ly;
    let initial = initialize(
        c,
        |x, y| {
            [
                cases::channel(y, ly, nu, force[0])
                    + a * 2. * ky * (kx * x).sin() * (ky * y).sin() * (ky * y).cos(),
                -a * kx * (kx * x).cos() * (ky * y).sin().powi(2),
            ]
        },
        |_, _| 1.,
        force,
    );
    let mut gpu = Gpu::new(ctx, c, &initial)?;
    let (mut field, _) = gpu.advance(&[], force, None)?;
    let init_ms = init.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    let mut cells = (count > 0).then(|| coupling::Cells::new(c, count));
    let cell_init = start.elapsed().as_secs_f64() * 1000.;
    let accelerations = vec![force; c.substeps];
    let mut csv = Vec::new();
    writeln!(
        csv,
        "frame,t,upload_ms,encode_ms,submit_ms,wait_ms,decode_ms,gpu_ms,advance_ms,coupling_ms,rapier_ms,diagnostics_ms,fluid_coupling_ms,whole_ms,rho_min,rho_max,rho_mean,max_mach,mean_u,max_area_rel,max_node_speed,cell_mean_vx,uploaded_exchange_mismatch"
    )?;
    let warmup = 30;
    let mut last_diag = json!({});
    for frame in 0..warmup + frames {
        let whole = Instant::now();
        let (mut timing, mut coupling_ms, mut rapier_ms, mut mismatch) =
            (Times::default(), 0., 0., 0_f64);
        for _ in 0..2 {
            let t = Instant::now();
            let kicks = if let Some(b) = cells.as_mut() {
                let (k, m) = b.exchange(c, &field);
                mismatch = mismatch.max(m);
                Some(k)
            } else {
                None
            };
            coupling_ms += t.elapsed().as_secs_f64() * 1000.;
            let (f, t) = gpu.advance(&accelerations, force, kicks.as_deref())?;
            field = f;
            timing += t;
            let t = Instant::now();
            if let Some(b) = cells.as_mut() {
                b.step(c.lx);
            }
            rapier_ms += t.elapsed().as_secs_f64() * 1000.;
        }
        let t = Instant::now();
        let d = field.diagnostics(c);
        let cd = cells
            .as_ref()
            .map_or((0., 0., 0.), coupling::Cells::diagnostics);
        let diagnostic_ms = t.elapsed().as_secs_f64() * 1000.;
        let whole_ms = whole.elapsed().as_secs_f64() * 1000.;
        if frame >= warmup {
            writeln!(
                csv,
                "{},{},{},{},{},{},{},{},{},{coupling_ms},{rapier_ms},{diagnostic_ms},{},{whole_ms},{},{},{},{},{},{},{},{},{mismatch}",
                frame - warmup,
                (frame + 1) as f64 / 60.,
                timing.upload,
                timing.encode,
                timing.submit,
                timing.wait,
                timing.decode,
                timing.gpu,
                timing.whole,
                timing.whole + coupling_ms,
                d["rho_min"],
                d["rho_max"],
                d["rho_mean"],
                d["max_mach"],
                d["mean_u"],
                cd.0,
                cd.1,
                cd.2
            )?;
        }
        last_diag = d;
    }
    std::fs::write(format!("{}.csv", prefix.display()), csv)?;
    let meta = json!({"config":c.metadata(),"cells":count,"frames":frames,"warmup_frames":warmup,"re_reference":re,"adapter":ctx.info,"timestamps":ctx.timestamps,"init_ms":init_ms,"cell_init_ms":cell_init,"diagnostics":last_diag,"readback_bytes_per_frame":2*(c.len()*16+16),"submissions_per_frame":2,"dispatches_per_frame":2*(c.substeps+1+usize::from(count>0)),"exchange":"explicit once per 1/120s; collocated bilinear; momentum kick","body_model":"12 nodes; radius 0.7; mass 1; 4 markers; gamma 0.25"});
    std::fs::write(
        format!("{}.json", prefix.display()),
        serde_json::to_string_pretty(&meta)?,
    )?;
    println!("{}", meta);
    Ok(())
}
fn main() -> Result<()> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build_global()?;
    let args: Vec<_> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("help");
    if mode == "help" {
        println!(
            "probe | compare OUTPUT | physics steady|pulse|vortex N RE REFINE OUTDIR | bench NX NY CELLS FRAMES RE PREFIX [--allow-software]"
        );
        return Ok(());
    }
    let ctx = Context::new(args.iter().any(|x| x == "--allow-software"))?;
    match mode {
        "probe" => println!(
            "{}",
            json!({"adapter":ctx.info,"timestamps":ctx.timestamps})
        ),
        "compare" => {
            let r = cases::compare(&ctx)?;
            std::fs::write(&args[2], serde_json::to_string_pretty(&r)?)?;
            println!("{r}");
        }
        "physics" => {
            let r = cases::physics_case(
                &ctx,
                &args[2],
                args[3].parse()?,
                args[4].parse()?,
                args[5].parse()?,
                Path::new(&args[6]),
            )?;
            println!("{r}");
        }
        "bench" => bench(
            &ctx,
            args[2].parse()?,
            args[3].parse()?,
            args[4].parse()?,
            args[5].parse()?,
            args[6].parse()?,
            Path::new(&args[7]),
        )?,
        _ => return Err("unknown command".into()),
    }
    Ok(())
}
