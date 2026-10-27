//! GPU backends: Stokes solvers and the optional simulation-facing LBM module.
pub mod lbm;
use blood_fluid::krylov as gmres;
mod legacy;
mod mg;
mod profile;
use blood_fluid::{
    DeviceInfo, FluidResult, PreparedSolver, SolveOptions, SolveReport, SolverCounters,
    SolverFactory, StokesProblem,
};
use serde_json::{Value, json};
use std::sync::Arc;
type LabResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct Factory {
    context: Arc<legacy::Context>,
    options: SolveOptions,
    multigrid: bool,
    profiling: bool,
}
pub fn factory(
    name: &str,
    options: SolveOptions,
    allow_software: bool,
) -> FluidResult<Arc<dyn SolverFactory>> {
    make_factory(name, options, allow_software, false, false)
}
/// Diagnostic entry point; timestamps are optional and never a game requirement.
pub fn factory_profiled(
    name: &str,
    options: SolveOptions,
    allow_software: bool,
    timestamps: bool,
) -> FluidResult<Arc<dyn SolverFactory>> {
    if name != "gpu-fgmres-mg" {
        return Err("profiling currently supports gpu-fgmres-mg".into());
    }
    make_factory(name, options, allow_software, true, timestamps)
}
fn make_factory(
    name: &str,
    options: SolveOptions,
    allow_software: bool,
    profiling: bool,
    timestamps: bool,
) -> FluidResult<Arc<dyn SolverFactory>> {
    if name != "gpu-gmres" && name != "gpu-fgmres-mg" {
        return Err("unknown GPU backend".into());
    }
    let context = pollster::block_on(legacy::Context::new_with_timestamps(
        allow_software,
        timestamps,
    ))
    .map_err(|e| e.to_string())?;
    Ok(Arc::new(Factory {
        context: Arc::new(context),
        options,
        multigrid: name == "gpu-fgmres-mg",
        profiling,
    }))
}
impl SolverFactory for Factory {
    fn name(&self) -> &'static str {
        if self.multigrid {
            "gpu-fgmres-mg"
        } else {
            "gpu-gmres"
        }
    }
    fn device(&self) -> Option<DeviceInfo> {
        let i = &self.context.info;
        Some(DeviceInfo {
            name: i.name.clone(),
            backend: i.backend.clone(),
            kind: i.device_type.clone(),
            software: i.software,
        })
    }
    fn prepare(&self, p: Arc<StokesProblem>) -> FluidResult<Box<dyn PreparedSolver>> {
        p.validate()?;
        if self.multigrid {
            return mg::MgSolver::new(self.context.clone(), p, self.options, self.profiling)
                .map(|s| Box::new(s) as Box<dyn PreparedSolver>)
                .map_err(|e| e.to_string());
        }
        let scale = p.scaling()?;
        let values: Vec<_> = p
            .matrix
            .scaled(&scale)
            .values
            .iter()
            .map(|&x| x as f32)
            .collect();
        let system = legacy::System::new(
            self.context.clone(),
            &p.matrix.ptr,
            &p.matrix.columns,
            &values,
            self.options.restart,
        )
        .map_err(|e| e.to_string())?;
        Ok(Box::new(LegacySolver {
            system,
            problem: p,
            scale,
            options: self.options,
        }))
    }
}
struct LegacySolver {
    system: legacy::System,
    problem: Arc<StokesProblem>,
    scale: Vec<f64>,
    options: SolveOptions,
}
impl PreparedSolver for LegacySolver {
    fn solve(&mut self, b: &[f64], x: &mut [f64]) -> FluidResult<SolveReport> {
        let result = (|| -> LabResult<SolveReport> {
            if b.len() != self.problem.matrix.rows || x.len() != b.len() {
                return Err("invalid vector length".into());
            }
            let rhs: Vec<f32> = b
                .iter()
                .zip(&self.scale)
                .map(|(b, d)| (b * d) as f32)
                .collect();
            self.system.reset(&rhs)?;
            let o = gmres::solve(
                &mut self.system,
                gmres::Config {
                    restart: self.options.restart,
                    max_iterations: self.options.max_iterations,
                    tolerance: self.options.tolerance,
                },
            )?;
            let y = self.system.solution()?;
            for i in 0..x.len() {
                x[i] = y[i] * self.scale[i];
            }
            let accepted = self
                .problem
                .audit(b, x, None, self.options.tolerance)
                .linear_residual_accepted;
            let status = if o.status == "converged_scaled" {
                if accepted {
                    "converged"
                } else {
                    "original_residual_failed"
                }
            } else {
                &o.status
            };
            let c = &self.system.counters;
            Ok(SolveReport {
                backend: "gpu-gmres".into(),
                status: status.into(),
                iterations: o.iterations,
                stopping_relative: Some(o.scaled_relative_residual),
                multigrid_levels: Vec::new(),
                counters: SolverCounters {
                    dispatches: c.dispatches,
                    submissions: c.dispatches + c.readbacks,
                    readbacks: c.readbacks,
                    readback_bytes: c.readback_bytes,
                    readback_wait_ms: c.readback_wait_ms,
                },
                profile: None,
            })
        })();
        result.map_err(|e| e.to_string())
    }
}
pub fn probe(allow_software: bool) -> FluidResult<Value> {
    let context =
        pollster::block_on(legacy::Context::new(allow_software)).map_err(|e| e.to_string())?;
    Ok(json!(context.info))
}
pub fn smoke(allow_software: bool) -> FluidResult<Value> {
    use blood_fluid::CsrMatrix;
    let options = SolveOptions {
        tolerance: 1e-5,
        restart: 32,
        max_iterations: 200,
        v_cycles: 2,
    };
    let context = Arc::new(
        pollster::block_on(legacy::Context::new(allow_software)).map_err(|e| e.to_string())?,
    );
    let new = Factory {
        context: context.clone(),
        options,
        multigrid: true,
        profiling: false,
    };
    let mut samples = Vec::new();
    let mut accepted = true;
    for nv in [2_usize, 129] {
        let (n, entries, reference) = if nv == 2 {
            (
                3,
                vec![
                    (0, 0, 2.),
                    (1, 1, 3.),
                    (0, 2, 1.),
                    (2, 0, 1.),
                    (1, 2, -1.),
                    (2, 1, -1.),
                ],
                vec![1., 1., 2.],
            )
        } else {
            let n = 2 * nv - 1;
            let mut entries = Vec::new();
            for i in 0..nv {
                entries.push((i, i, 2.));
                if i + 1 < nv {
                    entries.push((i, i + 1, -1.));
                    entries.push((i + 1, i, -1.));
                }
            }
            for i in 0..nv - 1 {
                entries.extend([
                    (nv + i, i, 1.),
                    (i, nv + i, 1.),
                    (nv + i, i + 1, -1.),
                    (i + 1, nv + i, -1.),
                ]);
            }
            let reference = (0..n).map(|i| if i < nv { 1. } else { 2. }).collect();
            (n, entries, reference)
        };
        let p = Arc::new(StokesProblem {
            matrix: CsrMatrix::from_triplets(n, &entries)?,
            velocity_count: nv,
            viscosity: 1.,
            geometry: None,
        });
        let b = p.matrix.apply(&reference);
        let mut solver = new.prepare(p.clone())?;
        let mut x = vec![0.; n];
        let report = solver.solve(&b, &mut x)?;
        let audit = p.audit(&b, &x, Some(&reference), options.tolerance);
        let max_error = x
            .iter()
            .zip(&reference)
            .map(|(x, r)| (x - r).abs())
            .fold(0., f64::max);
        let ok = report.converged() && audit.linear_residual_accepted && max_error < 0.005;
        accepted &= ok;
        samples.push(
            json!({"rows":n,"report":report,"audit":audit,"max_error":max_error,"accepted":ok}),
        );
        let zero = solver.solve(&vec![0.; n], &mut x)?;
        accepted &= zero.converged() && zero.iterations == 0 && x.iter().all(|x| *x == 0.);
        if nv == 2 {
            let old = Factory {
                context: context.clone(),
                options,
                multigrid: false,
                profiling: false,
            };
            let mut solver = old.prepare(p.clone())?;
            let r = solver.solve(&b, &mut x)?;
            accepted &= r.converged()
                && p.audit(&b, &x, Some(&reference), options.tolerance)
                    .linear_residual_accepted;
        }
    }
    Ok(
        json!({"mode":"smoke","adapter":context.info,"hardware_measurement":false,"samples":samples,"accepted":accepted}),
    )
}

#[cfg(test)]
mod lbm_contract_test {
    use super::lbm::{GridProfile, WindowOrigin, WindowSpec};

    #[test]
    fn lbm_window_profiles_and_block_shift() {
        for (profile, cells, shift) in [(GridProfile::Fine, 192, 64), (GridProfile::Coarse, 96, 32)]
        {
            let window = WindowSpec {
                origin: WindowOrigin {
                    block_x: -1,
                    block_y: 2,
                },
                profile,
            };
            assert_eq!(window.world_bounds(), [[-16.0, 32.0], [32.0, 80.0]]);
            assert_eq!(profile.cells_per_side(), cells);
            assert_eq!(profile.cells_per_block(), shift);
            let moved = WindowSpec {
                origin: WindowOrigin {
                    block_x: 0,
                    block_y: 2,
                },
                profile,
            };
            assert_eq!(moved.origin_cell_offset_from(window), [shift as i64, 0]);
        }
    }
}
