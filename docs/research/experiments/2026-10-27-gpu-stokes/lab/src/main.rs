mod case;
mod gmres;
mod gpu;
// Reuse the actual production LU implementation as the CPU reference.
#[path = "../../../../../../crates/simulation/src/fluid/linear.rs"]
#[allow(dead_code)]
mod production_lu;

use case::{Case, norm};
use serde_json::{Value, json};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};
type LabResult<T> = Result<T, Box<dyn std::error::Error>>;

fn value(args: &[String], flag: &str, default: &str) -> String {
    args.iter().position(|a| a == flag).map_or_else(
        || default.into(),
        |i| args.get(i + 1).expect("missing option value").clone(),
    )
}

fn cpu(case: &Case, limit: usize, repeats: usize, tolerance: f64) -> LabResult<Value> {
    let init = Instant::now();
    let factor = case.direct()?;
    let factor_ms = init.elapsed().as_secs_f64() * 1000.0;
    let mut rows = Vec::new();
    let mut accepted = true;
    for id in case.samples()?.into_iter().take(limit) {
        let (rhs, reference) = case.sample(&id)?;
        let _ = factor.solve_full(&rhs); // Untimed warm-up; factors remain fixed.
        let mut total = 0.0;
        let mut result = Vec::new();
        for _ in 0..repeats {
            let start = Instant::now();
            result = factor.solve_full(&rhs);
            total += start.elapsed().as_secs_f64() * 1000.0;
            std::hint::black_box(&result);
        }
        let audit = case.audit(&rhs, &result, &reference, tolerance);
        accepted &= audit.linear_residual_accepted;
        rows.push(json!({"sample":id,"mean_solve_ms":total/repeats as f64,"repeats":repeats,"audit":audit}));
    }
    Ok(
        json!({"mode":"cpu_lu_f64","factorization_ms":factor_ms,"samples":rows,
        "accepted":accepted,"source":"production StokesFactor; sequential faer sparse LU"}),
    )
}

fn gpu_case(case: &Case, args: &[String], limit: usize, tolerance: f64) -> LabResult<Value> {
    let config = gmres::Config {
        restart: value(args, "--restart", "32").parse()?,
        max_iterations: value(args, "--max-iter", "400").parse()?,
        tolerance,
    };
    let init = Instant::now();
    let context = pollster::block_on(gpu::Context::new(
        args.iter().any(|a| a == "--allow-software"),
    ))?;
    let scale = case.scaling()?;
    let values = case.scaled_values(&scale);
    let mut system = gpu::System::new(&context, &case.ptr, &case.columns, &values, config.restart)?;
    let setup_ms = init.elapsed().as_secs_f64() * 1000.0;
    // Same rounded f32 input, independently evaluate the original matrix in f64.
    let test: Vec<f32> = (0..case.meta.rows)
        .map(|i| ((i % 17) as f32 - 8.0) * 0.125)
        .collect();
    let input: Vec<_> = test
        .iter()
        .zip(&scale)
        .map(|(&x, d)| x as f64 * d)
        .collect();
    let expected: Vec<_> = case
        .multiply(&input)
        .iter()
        .zip(&scale)
        .map(|(x, d)| x * d)
        .collect();
    let spmv_start = Instant::now();
    let actual = system.spmv(&test)?;
    let spmv_ms = spmv_start.elapsed().as_secs_f64() * 1000.0;
    let error: Vec<_> = actual.iter().zip(&expected).map(|(a, b)| a - b).collect();
    let spmv_relative = norm(&error) / norm(&expected).max(1e-30);
    if !spmv_relative.is_finite() || spmv_relative > 1e-5 {
        return Err(format!("GPU SpMV mismatch: {spmv_relative:e}").into());
    }
    let mut rows = Vec::new();
    let mut accepted = true;
    for id in case.samples()?.into_iter().take(limit) {
        let (rhs, reference) = case.sample(&id)?;
        let scaled_rhs: Vec<_> = rhs
            .iter()
            .zip(&scale)
            .map(|(b, d)| (b * d) as f32)
            .collect();
        let start = Instant::now();
        system.reset(&scaled_rhs)?;
        let outcome = gmres::solve(&mut system, config)?;
        let y = system.solution()?;
        let total_ms = start.elapsed().as_secs_f64() * 1000.0;
        let x: Vec<_> = y.iter().zip(&scale).map(|(y, d)| y * d).collect();
        let audit = case.audit(&rhs, &x, &reference, tolerance);
        let ok = outcome.status == "converged_scaled" && audit.linear_residual_accepted;
        accepted &= ok;
        rows.push(
            json!({"sample":id,"end_to_end_solve_ms":total_ms,"outcome":outcome,
            "counters":system.counters,"audit":audit,"accepted":ok}),
        );
    }
    Ok(
        json!({"mode":"gpu_gmres_f32","adapter":context.info,"setup_ms":setup_ms,
        "hardware_measurement":!context.info.software,"restart":config.restart,"max_iterations":config.max_iterations,
        "spmv_roundtrip_ms":spmv_ms,"spmv_relative_error":spmv_relative,
        "samples":rows,"accepted":accepted,
        "notes":"Host scalar GMRES; GPU large vectors; includes per-iteration scalar readbacks and final vector readback; no Rapier or rendering"}),
    )
}

fn smoke(allow_software: bool) -> LabResult<Value> {
    let context = pollster::block_on(gpu::Context::new(allow_software))?;
    // K=[diag(2,3), B^T; B,0], x=[1,1,2], b=[4,1,0].
    let mut system = gpu::System::new(
        &context,
        &[0, 2, 4, 6],
        &[0, 2, 1, 2, 0, 1],
        &[2., 1., 3., -1., 1., -1.],
        3,
    )?;
    let product = system.spmv(&[1., 1., 2.])?;
    if product
        .iter()
        .zip([4., 1., 0.])
        .any(|(a, b)| (a - b).abs() > 1e-6)
    {
        return Err("smoke SpMV failed".into());
    }
    system.reset(&[4., 1., 0.])?;
    let config = gmres::Config {
        restart: 3,
        max_iterations: 12,
        tolerance: 1e-5,
    };
    let outcome = gmres::solve(&mut system, config)?;
    let solution = system.solution()?;
    let error = solution
        .iter()
        .zip([1., 1., 2.])
        .map(|(a, b)| (a - b).abs())
        .fold(0., f64::max);
    system.reset(&[0., 0., 0.])?;
    let zero = gmres::solve(&mut system, config)?;
    let zero_solution = system.solution()?;
    // A repeated Stokes block spans three reduction chunks, exercising the
    // inter-workgroup reduction rather than only the tiny 3x3 path.
    let pairs = 257;
    let mut ptr = vec![0_u32];
    let mut cols = Vec::new();
    let mut vals = Vec::new();
    for i in 0..pairs {
        cols.extend([i as u32, (i + pairs) as u32]);
        vals.extend([2., 1.]);
        ptr.push(cols.len() as u32);
    }
    for i in 0..pairs {
        cols.push(i as u32);
        vals.push(1.);
        ptr.push(cols.len() as u32);
    }
    let mut large = gpu::System::new(&context, &ptr, &cols, &vals, 4)?;
    let rhs: Vec<_> = (0..2 * pairs)
        .map(|i| if i < pairs { 4. } else { 1. })
        .collect();
    large.reset(&rhs)?;
    let large_outcome = gmres::solve(
        &mut large,
        gmres::Config {
            restart: 4,
            ..config
        },
    )?;
    let large_solution = large.solution()?;
    let large_error = large_solution
        .iter()
        .enumerate()
        .map(|(i, x)| (x - if i < pairs { 1. } else { 2. }).abs())
        .fold(0., f64::max);
    let accepted = outcome.status == "converged_scaled"
        && error < 1e-4
        && zero.iterations == 0
        && zero_solution.iter().all(|x| *x == 0.0);
    Ok(
        json!({"mode":"smoke","adapter":context.info,"hardware_measurement":false,
        "outcome":outcome,"solution":solution,"max_error":error,"zero_rhs":zero,
        "multi_chunk_outcome":large_outcome,"multi_chunk_max_error":large_error,
        "accepted":accepted && large_outcome.status=="converged_scaled" && large_error<1e-4}),
    )
}

fn run(args: &[String]) -> LabResult<Value> {
    let mode = args.get(1).map(String::as_str).unwrap_or("help");
    let allow = args.iter().any(|a| a == "--allow-software");
    if mode == "probe" {
        let context = pollster::block_on(gpu::Context::new(allow))?;
        return Ok(json!({"mode":"probe","adapter":context.info,"accepted":true}));
    }
    if mode == "smoke" {
        return smoke(allow);
    }
    if mode != "cpu" && mode != "gpu" {
        return Err("usage: blood-gpu-stokes-lab probe|smoke [--allow-software] OR cpu|gpu --case DIR [--limit 8 --repeats 10 --rtol 1e-4 --restart 32 --max-iter 400 --out NEW.json]".into());
    }
    let folder = value(args, "--case", "");
    if folder.is_empty() {
        return Err("--case is required".into());
    }
    let case = Case::load(Path::new(&folder))?;
    let limit: usize = value(args, "--limit", "8").parse()?;
    let repeats: usize = value(args, "--repeats", "10").parse()?;
    let tolerance: f64 = value(args, "--rtol", "0.0001").parse()?;
    if limit == 0 || repeats == 0 || !tolerance.is_finite() || tolerance <= 0.0 {
        return Err("invalid limit/repeats/tolerance".into());
    }
    let mut result = if mode == "cpu" {
        cpu(&case, limit, repeats, tolerance)?
    } else {
        gpu_case(&case, args, limit, tolerance)?
    };
    result["case"] = json!(folder);
    result["rows"] = json!(case.meta.rows);
    result["tolerance"] = json!(tolerance);
    result["known_model_issue"] = json!(
        "wide domains omit internal pump; algebra correctness does not establish physical correctness"
    );
    Ok(result)
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let result = run(&args);
    let (mut record, code) = match result {
        Ok(value) => {
            let code = if value["accepted"] == true { 0 } else { 2 };
            (value, code)
        }
        Err(error) => (json!({"accepted":false,"error":error.to_string()}), 1),
    };
    record["schema_version"] = json!(1);
    record["command"] = json!(args);
    let text = serde_json::to_string_pretty(&record).unwrap();
    println!("{text}");
    let out = value(&args, "--out", "");
    if !out.is_empty() {
        let path = PathBuf::from(out);
        let saved = (|| -> std::io::Result<()> {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            writeln!(file, "{text}")
        })();
        if let Err(error) = saved {
            eprintln!("save result: {error}");
            std::process::exit(1);
        }
    }
    std::process::exit(code);
}
