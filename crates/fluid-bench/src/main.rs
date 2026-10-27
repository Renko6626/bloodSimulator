mod capture;
use blood_fluid::{SolveOptions, SolverFactory, dataset::DatasetCase};
use serde_json::{Value, json};
use std::{io::Write, path::Path, sync::Arc, time::Instant};
type Result<T> = std::result::Result<T, String>;
fn value(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|s| s == flag)
        .and_then(|i| args.get(i + 1))
        .map_or(default.to_string(), Clone::clone)
}
fn factory(name: &str, options: SolveOptions, _args: &[String]) -> Result<Arc<dyn SolverFactory>> {
    let profile = value(_args, "--profile", "off");
    if !["off", "host", "gpu"].contains(&profile.as_str()) {
        return Err("--profile must be off, host, or gpu".into());
    }
    if profile != "off" {
        #[cfg(feature = "gpu")]
        return blood_fluid_gpu::factory_profiled(
            name,
            options,
            _args.iter().any(|a| a == "--allow-software"),
            profile == "gpu",
        );
        #[cfg(not(feature = "gpu"))]
        return Err("profiling requires --features gpu".into());
    }
    match name {
        "cpu-lu" => Ok(Arc::new(blood_fluid::cpu::LuFactory)),
        "cpu-fgmres-mg" => Ok(Arc::new(blood_fluid::cpu::MultigridFactory { options })),
        #[cfg(feature = "gpu")]
        "gpu-gmres" | "gpu-fgmres-mg" => {
            blood_fluid_gpu::factory(name, options, _args.iter().any(|a| a == "--allow-software"))
        }
        _ => Err(format!(
            "unknown/unavailable backend {name}; GPU requires --features gpu"
        )),
    }
}
fn run(args: &[String]) -> Result<Value> {
    let mode = args.get(1).map(String::as_str).unwrap_or("help");
    if mode == "capture" {
        let out = value(args, "--out", "");
        if out.is_empty() {
            return Err("capture requires --out".into());
        }
        let case = value(args, "--case", "small64");
        let frames = value(args, "--frames", "240")
            .parse::<usize>()
            .map_err(|e| e.to_string())?;
        capture::capture(out.into(), &case, frames)?;
        return Ok(json!({"accepted":true,"mode":"capture","case":case,"frames":frames}));
    }
    #[cfg(feature = "gpu")]
    if mode == "probe" || mode == "smoke" {
        let allow = args.iter().any(|a| a == "--allow-software");
        return if mode == "probe" {
            blood_fluid_gpu::probe(allow).map(|info| json!({"accepted":true,"adapter":info}))
        } else {
            blood_fluid_gpu::smoke(allow)
        };
    }
    if mode != "run" {
        return Err("usage: fluid-bench run --backend cpu-lu|cpu-fgmres-mg|gpu-gmres|gpu-fgmres-mg --case DIR [--limit 8 --restart 32 --max-iter 200 --rtol 1e-4 --out NEW.json]; capture --case small64|wide96|wide192 --out DIR; probe; smoke".into());
    }
    let folder = value(args, "--case", "");
    if folder.is_empty() {
        return Err("run requires --case".into());
    }
    let case = DatasetCase::load(Path::new(&folder))?;
    let options = SolveOptions {
        tolerance: value(args, "--rtol", "1e-4")
            .parse::<f64>()
            .map_err(|e| e.to_string())?,
        restart: value(args, "--restart", "32")
            .parse::<usize>()
            .map_err(|e| e.to_string())?,
        max_iterations: value(args, "--max-iter", "200")
            .parse::<usize>()
            .map_err(|e| e.to_string())?,
        v_cycles: value(args, "--mg-cycles", "2")
            .parse::<usize>()
            .map_err(|e| e.to_string())?,
    };
    let limit = value(args, "--limit", "8")
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    let repeats = value(args, "--repeats", "1")
        .parse::<usize>()
        .map_err(|e| e.to_string())?;
    if limit == 0
        || repeats == 0
        || options.restart == 0
        || options.restart > 64
        || options.max_iterations == 0
        || !options.tolerance.is_finite()
        || options.tolerance <= 0.
    {
        return Err("invalid solver options".into());
    }
    let name = value(args, "--backend", "cpu-lu");
    let start = Instant::now();
    let factory = factory(&name, options, args)?;
    let mut prepared = factory.prepare(case.problem.clone())?;
    let setup_ms = start.elapsed().as_secs_f64() * 1000.;
    let mut samples = Vec::new();
    let mut accepted = true;
    for id in case.samples()?.into_iter().take(limit) {
        let (b, reference) = case.sample(&id)?;
        let mut x = vec![0.; b.len()];
        let mut duration = 0.;
        let mut report = None;
        for _ in 0..repeats {
            let now = Instant::now();
            report = Some(prepared.solve(&b, &mut x)?);
            duration += now.elapsed().as_secs_f64() * 1000.;
        }
        let report = report.unwrap();
        let audit = case
            .problem
            .audit(&b, &x, Some(&reference), options.tolerance);
        let ok = report.converged() && audit.linear_residual_accepted;
        accepted &= ok;
        samples.push(json!({"sample":id,"mean_solve_ms":duration/repeats as f64,"report":report,"audit":audit,"accepted":ok}));
    }
    Ok(
        json!({"mode":"run","backend":name,"adapter":factory.device(),"hardware_measurement":factory.device().map(|d|!d.software).unwrap_or(false),"case":folder,"rows":case.problem.matrix.rows,"setup_ms":setup_ms,
        "options":options,"samples":samples,"accepted":accepted,"notes":"Linear-system replay only; no Rapier/rendering timing. Wide-domain pump omission unchanged."}),
    )
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let (mut record, exit) = match run(&args) {
        Ok(v) => {
            let code = if v["accepted"] == true { 0 } else { 2 };
            (v, code)
        }
        Err(e) => (json!({"accepted":false,"error":e}), 1),
    };
    record["schema_version"] = json!(2);
    record["command"] = json!(args);
    let text = serde_json::to_string_pretty(&record).unwrap();
    println!("{text}");
    let out = value(&args, "--out", "");
    if !out.is_empty() && args.get(1).map(String::as_str) != Some("capture") {
        let write = (|| -> std::io::Result<()> {
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(out)?;
            writeln!(f, "{text}")
        })();
        if let Err(e) = write {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
    std::process::exit(exit);
}
