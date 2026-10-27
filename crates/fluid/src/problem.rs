use crate::{CsrMatrix, FluidResult, matrix::norm};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Geometry {
    pub n: usize,
    pub h: f64,
    pub origin: [f64; 2],
    pub pressure_cells: Vec<usize>,
    pub u_positions: Vec<[f64; 2]>,
    pub v_positions: Vec<[f64; 2]>,
    #[serde(default = "all_open")]
    pub openings: [bool; 4],
}
fn all_open() -> [bool; 4] {
    [true; 4]
}

#[derive(Clone, Debug)]
pub struct StokesProblem {
    pub matrix: CsrMatrix,
    pub velocity_count: usize,
    pub viscosity: f64,
    pub geometry: Option<Geometry>,
}
impl StokesProblem {
    pub fn validate(&self) -> FluidResult<()> {
        self.matrix.validate()?;
        if self.matrix.cols != self.matrix.rows
            || self.velocity_count == 0
            || self.velocity_count >= self.matrix.rows
            || !self.viscosity.is_finite()
            || self.viscosity <= 0.
        {
            return Err("invalid Stokes split or viscosity".into());
        }
        if let Some(g) = &self.geometry {
            if g.u_positions.len() + g.v_positions.len() != self.velocity_count
                || g.pressure_cells.len() != self.matrix.rows - self.velocity_count
            {
                return Err("domain geometry does not match unknown ordering".into());
            }
        }
        Ok(())
    }
    pub fn scaling(&self) -> FluidResult<Vec<f64>> {
        let nv = self.velocity_count;
        let mut diag = self.matrix.diagonal();
        if diag[..nv].iter().any(|&a| a <= 0.) {
            return Err("velocity diagonal must be positive".into());
        }
        for i in nv..self.matrix.rows {
            diag[i] = self
                .matrix
                .row(i)
                .filter(|(j, _)| *j < nv)
                .map(|(j, a)| a * a / diag[j])
                .sum();
        }
        if diag.iter().any(|&a| a <= 0. || !a.is_finite()) {
            return Err("invalid block scaling".into());
        }
        Ok(diag.iter().map(|a| 1. / a.sqrt()).collect())
    }
    pub fn audit(&self, b: &[f64], x: &[f64], reference: Option<&[f64]>, tol: f64) -> Audit {
        let nv = self.velocity_count;
        let r: Vec<_> = self
            .matrix
            .apply(x)
            .iter()
            .zip(b)
            .map(|(a, b)| a - b)
            .collect();
        let relative = norm(&r) / norm(b).max(1e-30);
        let momentum_rms = norm(&r[..nv]) / (nv as f64).sqrt();
        let constraint_rms = norm(&r[nv..]) / ((self.matrix.rows - nv) as f64).sqrt();
        let mr = momentum_rms / (norm(&b[..nv]) / (nv as f64).sqrt()).max(1.);
        let finite = x.iter().chain(&r).all(|v| v.is_finite());
        let diff = reference.map(|refx| x.iter().zip(refx).map(|(x, r)| x - r).collect::<Vec<_>>());
        Audit {
            relative_residual: relative,
            momentum_rms,
            momentum_relative_rms: mr,
            constraint_rms,
            finite,
            linear_residual_accepted: finite
                && relative <= tol
                && mr <= tol
                && constraint_rms <= tol,
            velocity_reference_relative_l2: diff
                .as_ref()
                .map(|d| norm(&d[..nv]) / norm(&reference.unwrap()[..nv]).max(1e-30)),
            pressure_reference_max_absolute: diff
                .as_ref()
                .map(|d| d[nv..].iter().map(|x| x.abs()).fold(0., f64::max)),
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Audit {
    pub relative_residual: f64,
    pub momentum_rms: f64,
    pub momentum_relative_rms: f64,
    pub constraint_rms: f64,
    pub finite: bool,
    pub linear_residual_accepted: bool,
    pub velocity_reference_relative_l2: Option<f64>,
    pub pressure_reference_max_absolute: Option<f64>,
}
