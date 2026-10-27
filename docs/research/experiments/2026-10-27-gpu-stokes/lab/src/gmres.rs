//! Restarted GMRES control. Large vectors/Arnoldi live in the selected backend.
use crate::LabResult;
use serde::Serialize;

pub trait Krylov {
    fn rhs_norm(&self) -> f64;
    fn start_cycle(&mut self) -> LabResult<f64>;
    /// Returns h[0..=j+1] after two-pass orthogonalization and stores v[j+1].
    fn arnoldi(&mut self, j: usize) -> LabResult<Vec<f64>>;
    fn update(&mut self, coefficients: &[f64]) -> LabResult<()>;
}

#[derive(Clone, Copy)]
pub struct Config {
    pub restart: usize,
    pub max_iterations: usize,
    pub tolerance: f64,
}

#[derive(Serialize)]
pub struct Outcome {
    pub status: String,
    pub iterations: usize,
    pub cycles: usize,
    pub scaled_relative_residual: f64,
}

pub fn solve(backend: &mut impl Krylov, config: Config) -> LabResult<Outcome> {
    if config.restart == 0
        || config.restart > 64
        || config.max_iterations == 0
        || !config.tolerance.is_finite()
        || config.tolerance <= 0.0
    {
        return Err(
            "restart must be 1..64, iteration budget positive, tolerance positive/finite".into(),
        );
    }
    let rhs_norm = backend.rhs_norm();
    let denominator = if rhs_norm == 0.0 { 1.0 } else { rhs_norm };
    let target = config.tolerance * denominator;
    let mut iterations = 0;
    let mut cycles = 0;
    let mut broke_down = false;
    loop {
        let beta = backend.start_cycle()?;
        if !beta.is_finite() {
            return Err("non-finite true scaled residual".into());
        }
        let status = if beta <= target {
            Some("converged_scaled")
        } else if broke_down {
            Some("breakdown")
        } else if iterations >= config.max_iterations {
            Some("iteration_limit")
        } else {
            None
        };
        if let Some(status) = status {
            return Ok(Outcome {
                status: status.into(),
                iterations,
                cycles,
                scaled_relative_residual: beta / denominator,
            });
        }
        cycles += 1;
        let m = config.restart.min(config.max_iterations - iterations);
        let mut h = vec![vec![0.0; m]; m + 1];
        let mut c = vec![0.0; m];
        let mut s = vec![0.0; m];
        let mut g = vec![0.0; m + 1];
        g[0] = beta;
        let mut used = 0;
        for j in 0..m {
            let column = backend.arnoldi(j)?;
            if column.len() != j + 2 || column.iter().any(|v| !v.is_finite()) {
                return Err("invalid Arnoldi column".into());
            }
            for i in 0..=j + 1 {
                h[i][j] = column[i];
            }
            for i in 0..j {
                let upper = c[i] * h[i][j] + s[i] * h[i + 1][j];
                h[i + 1][j] = -s[i] * h[i][j] + c[i] * h[i + 1][j];
                h[i][j] = upper;
            }
            let length = h[j][j].hypot(h[j + 1][j]);
            if length <= f64::MIN_POSITIVE {
                broke_down = true;
                break;
            }
            c[j] = h[j][j] / length;
            s[j] = h[j + 1][j] / length;
            h[j][j] = length;
            h[j + 1][j] = 0.0;
            g[j + 1] = -s[j] * g[j];
            g[j] *= c[j];
            iterations += 1;
            used = j + 1;
            if g[j + 1].abs() <= target || column[j + 1] <= f64::MIN_POSITIVE {
                break;
            }
        }
        if used == 0 {
            broke_down = true;
            continue;
        }
        let mut y = vec![0.0; used];
        for i in (0..used).rev() {
            let remainder: f64 = (i + 1..used).map(|j| h[i][j] * y[j]).sum();
            y[i] = (g[i] - remainder) / h[i][i];
        }
        backend.update(&y)?;
        // Recompute the true residual at every restart or estimated convergence.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Dense {
        a: Vec<Vec<f64>>,
        b: Vec<f64>,
        x: Vec<f64>,
        basis: Vec<Vec<f64>>,
    }
    fn dot(a: &[f64], b: &[f64]) -> f64 {
        a.iter().zip(b).map(|(a, b)| a * b).sum()
    }
    impl Krylov for Dense {
        fn rhs_norm(&self) -> f64 {
            dot(&self.b, &self.b).sqrt()
        }
        fn start_cycle(&mut self) -> LabResult<f64> {
            let r: Vec<_> = self
                .a
                .iter()
                .zip(&self.b)
                .map(|(row, b)| b - dot(row, &self.x))
                .collect();
            let beta = dot(&r, &r).sqrt();
            self.basis = vec![
                r.iter()
                    .map(|r| if beta > 0.0 { r / beta } else { 0.0 })
                    .collect(),
            ];
            Ok(beta)
        }
        fn arnoldi(&mut self, j: usize) -> LabResult<Vec<f64>> {
            let mut w: Vec<_> = self.a.iter().map(|row| dot(row, &self.basis[j])).collect();
            let mut h = vec![0.0; j + 2];
            for _ in 0..2 {
                for (i, v) in self.basis.iter().enumerate() {
                    let d = dot(&w, v);
                    h[i] += d;
                    for (w, v) in w.iter_mut().zip(v) {
                        *w -= d * v;
                    }
                }
            }
            let length = dot(&w, &w).sqrt();
            h[j + 1] = length;
            self.basis.push(
                w.iter()
                    .map(|w| if length > 0.0 { w / length } else { 0.0 })
                    .collect(),
            );
            Ok(h)
        }
        fn update(&mut self, y: &[f64]) -> LabResult<()> {
            for (i, c) in y.iter().enumerate() {
                for (x, v) in self.x.iter_mut().zip(&self.basis[i]) {
                    *x += c * v;
                }
            }
            Ok(())
        }
    }
    #[test]
    fn solves_indefinite_system_and_handles_zero_rhs() {
        // Eigenvalues 1±sqrt(2): indefinite, with a zero pressure diagonal.
        let a = vec![vec![2.0, 1.0], vec![1.0, 0.0]];
        let mut op = Dense {
            a: a.clone(),
            b: vec![5.0, 1.0],
            x: vec![0.0; 2],
            basis: vec![],
        };
        let config = Config {
            restart: 2,
            max_iterations: 8,
            tolerance: 1e-12,
        };
        let out = solve(&mut op, config).unwrap();
        assert_eq!(out.status, "converged_scaled");
        assert!((op.x[0] - 1.0).abs() < 1e-10 && (op.x[1] - 3.0).abs() < 1e-10);
        let mut zero = Dense {
            a,
            b: vec![0.0; 2],
            x: vec![0.0; 2],
            basis: vec![],
        };
        let out = solve(&mut zero, config).unwrap();
        assert_eq!(out.iterations, 0);
        assert_eq!(zero.x, vec![0.0; 2]);
    }
}
