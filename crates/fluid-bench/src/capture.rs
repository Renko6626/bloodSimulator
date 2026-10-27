use blood_fluid::{
    FluidResult, PreparedSolver, SolveReport, SolverFactory, StokesProblem, dataset,
};
use std::{
    io::Write,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Instant,
};
struct CaptureFactory {
    root: PathBuf,
    next: AtomicUsize,
}
impl SolverFactory for CaptureFactory {
    fn name(&self) -> &'static str {
        "capture-cpu-lu"
    }
    fn prepare(&self, p: Arc<StokesProblem>) -> FluidResult<Box<dyn PreparedSolver>> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let root = self.root.join(format!("region-{id}"));
        dataset::write_problem(&root, &p)?;
        std::fs::write(root.join("samples.csv"), "sample,capture_cpu_solve_ms\n")
            .map_err(|e| e.to_string())?;
        let inner = blood_fluid::cpu::LuFactory.prepare(p)?;
        Ok(Box::new(Capture {
            inner,
            root,
            calls: 0,
        }))
    }
}
struct Capture {
    inner: Box<dyn PreparedSolver>,
    root: PathBuf,
    calls: usize,
}
impl PreparedSolver for Capture {
    fn solve(&mut self, b: &[f64], x: &mut [f64]) -> FluidResult<SolveReport> {
        let start = Instant::now();
        let r = self.inner.solve(b, x)?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.;
        let call = self.calls;
        self.calls += 1;
        if [0, 1, 3, 7, 15, 31, 63, 127].contains(&call) {
            dataset::write_f64(&self.root.join(format!("rhs-{call:03}.f64")), b)?;
            dataset::write_f64(&self.root.join(format!("solution-{call:03}.f64")), x)?;
            let mut log = std::fs::OpenOptions::new()
                .append(true)
                .open(self.root.join("samples.csv"))
                .map_err(|e| e.to_string())?;
            writeln!(log, "{call},{elapsed:.9}").map_err(|e| e.to_string())?;
        }
        Ok(r)
    }
}
pub fn capture(out: PathBuf, case: &str, frames: usize) -> FluidResult<()> {
    use simulation::{Config, LocalFluidOptions, MoveInput, Simulation};
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    if out.join("region-0").exists() {
        return Err("capture output already contains a region".into());
    }
    let factory = Arc::new(CaptureFactory {
        root: out,
        next: AtomicUsize::new(0),
    });
    let options = match case {
        "small64" => LocalFluidOptions {
            solver: factory,
            ..Default::default()
        },
        "wide96" => LocalFluidOptions {
            solver: factory,
            n: 96,
            h: 0.5,
            origins: [[-38., -24.], [-34., -32.]],
        },
        "wide192" => LocalFluidOptions {
            solver: factory,
            n: 192,
            h: 0.25,
            origins: [[-38., -24.], [-34., -32.]],
        },
        _ => return Err("capture case must be small64, wide96, or wide192".into()),
    };
    let mut sim = Simulation::new_local_world_with_options(Config::default(), options);
    let targets = [[-13., 0.], [-10., -4.], [-6., -8.], [-10., -4.], [-16., 0.]];
    let mut target = 0;
    for _ in 0..frames {
        let p = sim.snapshot().robot;
        let mut d = [targets[target][0] - p[0], targets[target][1] - p[1]];
        if d[0].hypot(d[1]) < 0.6 {
            target = (target + 1) % targets.len();
            d = [targets[target][0] - p[0], targets[target][1] - p[1]];
        }
        let length = d[0].hypot(d[1]).max(1.);
        sim.tick(MoveInput {
            x: d[0] / length,
            y: d[1] / length,
        });
    }
    Ok(())
}
