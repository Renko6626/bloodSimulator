use crate::{
    FluidResult, PreparedSolver, SolveOptions, SolveReport, SolverCounters, SolverFactory,
    StokesProblem, lu::StokesFactor,
};
use std::sync::Arc;

#[derive(Clone, Copy, Default)]
pub struct LuFactory;
impl SolverFactory for LuFactory {
    fn name(&self) -> &'static str {
        "cpu-lu"
    }
    fn prepare(&self, p: Arc<StokesProblem>) -> FluidResult<Box<dyn PreparedSolver>> {
        p.validate()?;
        let entries: Vec<_> = (0..p.matrix.rows)
            .flat_map(|i| {
                p.matrix
                    .row(i)
                    .map(move |(j, a)| faer::sparse::Triplet::new(i, j, a))
            })
            .collect();
        let lu = StokesFactor::new(p.velocity_count, p.matrix.rows - p.velocity_count, &entries)?;
        Ok(Box::new(LuSolver { lu }))
    }
}
struct LuSolver {
    lu: StokesFactor,
}
impl PreparedSolver for LuSolver {
    fn solve(&mut self, b: &[f64], x: &mut [f64]) -> FluidResult<SolveReport> {
        if x.len() != b.len() {
            return Err("invalid output length".into());
        }
        x.copy_from_slice(&self.lu.solve_full(b));
        Ok(SolveReport {
            backend: "cpu-lu".into(),
            status: "converged".into(),
            iterations: 0,
            stopping_relative: None,
            multigrid_levels: Vec::new(),
            counters: SolverCounters::default(),
            profile: None,
        })
    }
}

#[derive(Clone, Copy, Default)]
pub struct MultigridFactory {
    pub options: SolveOptions,
}
impl SolverFactory for MultigridFactory {
    fn name(&self) -> &'static str {
        "cpu-fgmres-mg"
    }
    fn prepare(&self, p: Arc<StokesProblem>) -> FluidResult<Box<dyn PreparedSolver>> {
        Ok(Box::new(crate::block::BlockSolver::new(p, self.options)?))
    }
}
