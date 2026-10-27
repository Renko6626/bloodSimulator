//! Right block-triangular preconditioning and CPU FGMRES reference.
use crate::{
    CsrMatrix, FluidResult, PreparedSolver, SolveOptions, SolveReport, SolverCounters,
    StokesProblem,
    krylov::{self, Krylov},
    matrix::{dot, norm},
    multigrid::Hierarchy,
};
use std::sync::Arc;

pub struct BlockPreconditioner {
    pub matrix: CsrMatrix,
    pub scale: Vec<f64>,
    pub velocity_count: usize,
    pub hierarchy: Hierarchy,
    pub pressure_inverse: Vec<f64>,
    pub cycles: usize,
}
impl BlockPreconditioner {
    pub fn new(p: &StokesProblem) -> FluidResult<Self> {
        Self::with_cycles(p, 2)
    }
    pub fn with_cycles(p: &StokesProblem, cycles: usize) -> FluidResult<Self> {
        if !(1..=4).contains(&cycles) {
            return Err("V-cycle count must be 1..4".into());
        }
        p.validate()?;
        let scale = p.scaling()?;
        let matrix = p.matrix.scaled(&scale);
        let nv = p.velocity_count;
        let (positions, components, width) = if let Some(g) = &p.geometry {
            let mut positions = g.u_positions.clone();
            positions.extend(&g.v_positions);
            (
                Some(positions),
                (0..nv)
                    .map(|i| u8::from(i >= g.u_positions.len()))
                    .collect(),
                g.h * 2.,
            )
        } else {
            (None, vec![0; nv], 1.)
        };
        let hierarchy = Hierarchy::build(matrix.leading(nv)?, positions, components, width, 64)?;
        let pressure_inverse = scale[nv..].iter().map(|d| p.viscosity / (d * d)).collect();
        Ok(Self {
            matrix,
            scale,
            velocity_count: nv,
            hierarchy,
            pressure_inverse,
            cycles,
        })
    }
    pub fn apply(&self, r: &[f64]) -> Vec<f64> {
        let nv = self.velocity_count;
        let mut z = vec![0.; r.len()];
        for i in nv..r.len() {
            z[i] = -self.pressure_inverse[i - nv] * r[i];
        }
        let mut t = r[..nv].to_vec();
        for i in 0..nv {
            for (j, a) in self.matrix.row(i) {
                if j >= nv {
                    t[i] -= a * z[j];
                }
            }
        }
        z[..nv].copy_from_slice(&self.hierarchy.apply_cycles(&t, self.cycles));
        z
    }
}

pub struct BlockSolver {
    problem: Arc<StokesProblem>,
    pub preconditioner: BlockPreconditioner,
    options: SolveOptions,
}
impl BlockSolver {
    pub fn new(problem: Arc<StokesProblem>, options: SolveOptions) -> FluidResult<Self> {
        let preconditioner = BlockPreconditioner::with_cycles(&problem, options.v_cycles)?;
        Ok(Self {
            problem,
            preconditioner,
            options,
        })
    }
}
struct Session<'a> {
    pre: &'a BlockPreconditioner,
    b: Vec<f64>,
    x: Vec<f64>,
    v: Vec<Vec<f64>>,
    z: Vec<Vec<f64>>,
}
impl Krylov for Session<'_> {
    fn rhs_norm(&self) -> f64 {
        norm(&self.b)
    }
    fn start_cycle(&mut self) -> FluidResult<f64> {
        let r: Vec<_> = self
            .b
            .iter()
            .zip(self.pre.matrix.apply(&self.x))
            .map(|(b, a)| b - a)
            .collect();
        let beta = norm(&r);
        self.v = vec![
            r.iter()
                .map(|r| if beta > 0. { r / beta } else { 0. })
                .collect(),
        ];
        self.z.clear();
        Ok(beta)
    }
    fn arnoldi(&mut self, j: usize) -> FluidResult<Vec<f64>> {
        let z = self.pre.apply(&self.v[j]);
        let mut w = self.pre.matrix.apply(&z);
        self.z.push(z);
        let mut h = vec![0.; j + 2];
        for _ in 0..2 {
            let pass: Vec<_> = self.v.iter().map(|v| dot(&w, v)).collect();
            for i in 0..w.len() {
                w[i] -= self.v.iter().zip(&pass).map(|(v, d)| v[i] * d).sum::<f64>();
            }
            for i in 0..=j {
                h[i] += pass[i];
            }
        }
        let length = norm(&w);
        h[j + 1] = length;
        self.v.push(
            w.iter()
                .map(|w| if length > 0. { w / length } else { 0. })
                .collect(),
        );
        Ok(h)
    }
    fn update(&mut self, y: &[f64]) -> FluidResult<()> {
        for i in 0..self.x.len() {
            self.x[i] += y.iter().zip(&self.z).map(|(c, z)| c * z[i]).sum::<f64>();
        }
        Ok(())
    }
}
impl PreparedSolver for BlockSolver {
    fn solve(&mut self, b: &[f64], x: &mut [f64]) -> FluidResult<SolveReport> {
        if b.len() != self.problem.matrix.rows || x.len() != b.len() {
            return Err("invalid solver vector length".into());
        }
        let rhs = b
            .iter()
            .zip(&self.preconditioner.scale)
            .map(|(b, d)| b * d)
            .collect();
        let mut s = Session {
            pre: &self.preconditioner,
            b: rhs,
            x: vec![0.; b.len()],
            v: Vec::new(),
            z: Vec::new(),
        };
        let o = krylov::solve(
            &mut s,
            krylov::Config {
                restart: self.options.restart,
                max_iterations: self.options.max_iterations,
                tolerance: self.options.tolerance,
            },
        )?;
        for i in 0..x.len() {
            x[i] = s.x[i] * self.preconditioner.scale[i];
        }
        let audit = self.problem.audit(b, x, None, self.options.tolerance);
        let status = if o.status == "converged_scaled" {
            if audit.linear_residual_accepted {
                "converged"
            } else {
                "original_residual_failed"
            }
        } else {
            &o.status
        };
        Ok(SolveReport {
            backend: "cpu-fgmres-mg".into(),
            status: status.into(),
            iterations: o.iterations,
            stopping_relative: Some(o.scaled_relative_residual),
            multigrid_levels: self
                .preconditioner
                .hierarchy
                .levels
                .iter()
                .map(|l| l.matrix.rows)
                .collect(),
            counters: SolverCounters::default(),
            profile: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fgmres_uses_preconditioned_directions_on_indefinite_system() {
        let matrix = CsrMatrix::from_triplets(
            3,
            &[
                (0, 0, 2.),
                (1, 1, 3.),
                (0, 2, 1.),
                (2, 0, 1.),
                (1, 2, -1.),
                (2, 1, -1.),
            ],
        )
        .unwrap();
        let p = Arc::new(StokesProblem {
            matrix,
            velocity_count: 2,
            viscosity: 1.,
            geometry: None,
        });
        let mut solver = BlockSolver::new(
            p.clone(),
            SolveOptions {
                tolerance: 1e-10,
                ..Default::default()
            },
        )
        .unwrap();
        let mut x = vec![0.; 3];
        let report = solver.solve(&[4., 1., 0.], &mut x).unwrap();
        assert!(report.converged());
        assert!(
            x.iter()
                .zip([1., 1., 2.])
                .all(|(x, y)| (x - y).abs() < 1e-8)
        );
    }
}
