use crate::{LabResult, production_lu::StokesFactor};
use faer::sparse::Triplet;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct Metadata {
    pub format_version: u32,
    pub rows: usize,
    pub velocity_count: usize,
    pub pressure_count: usize,
    pub nnz: usize,
}

pub struct Case {
    pub meta: Metadata,
    pub ptr: Vec<u32>,
    pub columns: Vec<u32>,
    pub values: Vec<f64>,
    pub folder: PathBuf,
}

pub fn read_f64(path: &Path) -> LabResult<Vec<f64>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() % 8 != 0 {
        return Err(format!("invalid f64 array: {}", path.display()).into());
    }
    let values: Vec<_> = bytes
        .chunks_exact(8)
        .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
        .collect();
    if values.iter().any(|v| !v.is_finite()) {
        return Err(format!("non-finite input: {}", path.display()).into());
    }
    Ok(values)
}

fn read_u32(path: &Path) -> LabResult<Vec<u32>> {
    let bytes = std::fs::read(path)?;
    if bytes.len() % 4 != 0 {
        return Err(format!("invalid u32 array: {}", path.display()).into());
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect())
}

pub fn norm(x: &[f64]) -> f64 {
    x.iter().map(|v| v * v).sum::<f64>().sqrt()
}

impl Case {
    pub fn load(folder: &Path) -> LabResult<Self> {
        let meta: Metadata = serde_json::from_slice(&std::fs::read(folder.join("matrix.json"))?)?;
        let ptr = read_u32(&folder.join("row_ptr.u32"))?;
        let columns = read_u32(&folder.join("columns.u32"))?;
        let values = read_f64(&folder.join("values.f64"))?;
        if meta.format_version != 1
            || meta.rows != meta.velocity_count + meta.pressure_count
            || meta.rows == 0
            || ptr.len() != meta.rows + 1
            || ptr[0] != 0
            || ptr.last().copied() != Some(meta.nnz as u32)
            || columns.len() != meta.nnz
            || values.len() != meta.nnz
            || ptr.windows(2).any(|p| p[0] > p[1])
            || columns.iter().any(|&c| c as usize >= meta.rows)
        {
            return Err("invalid CSR dimensions or indices".into());
        }
        Ok(Self {
            meta,
            ptr,
            columns,
            values,
            folder: folder.into(),
        })
    }

    pub fn samples(&self) -> LabResult<Vec<String>> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&self.folder)? {
            let name = entry?.file_name().to_string_lossy().into_owned();
            if let Some(id) = name
                .strip_prefix("rhs-")
                .and_then(|s| s.strip_suffix(".f64"))
            {
                names.push(id.to_string());
            }
        }
        names.sort();
        if names.is_empty() {
            return Err("no captured RHSs".into());
        }
        Ok(names)
    }

    pub fn sample(&self, id: &str) -> LabResult<(Vec<f64>, Vec<f64>)> {
        let b = read_f64(&self.folder.join(format!("rhs-{id}.f64")))?;
        let x = read_f64(&self.folder.join(format!("solution-{id}.f64")))?;
        if b.len() != self.meta.rows || x.len() != self.meta.rows {
            return Err("captured RHS/reference length does not match matrix".into());
        }
        Ok((b, x))
    }

    pub fn multiply(&self, x: &[f64]) -> Vec<f64> {
        (0..self.meta.rows)
            .map(|i| {
                (self.ptr[i] as usize..self.ptr[i + 1] as usize)
                    .map(|k| self.values[k] * x[self.columns[k] as usize])
                    .sum()
            })
            .collect()
    }

    pub fn direct(&self) -> LabResult<StokesFactor> {
        let entries: Vec<_> = (0..self.meta.rows)
            .flat_map(|i| {
                (self.ptr[i] as usize..self.ptr[i + 1] as usize)
                    .map(move |k| Triplet::new(i, self.columns[k] as usize, self.values[k]))
            })
            .collect();
        StokesFactor::new(self.meta.velocity_count, self.meta.pressure_count, &entries)
            .map_err(Into::into)
    }

    /// K' = D K D, b' = D b, x = D y. No pressure penalty or equation change.
    /// Velocity scaling uses diag(A); pressure scaling uses diag(B diag(A)^-1 B^T).
    pub fn scaling(&self) -> LabResult<Vec<f64>> {
        let n = self.meta.rows;
        let nv = self.meta.velocity_count;
        let mut diagonal = vec![0.0; n];
        for (i, d) in diagonal.iter_mut().enumerate().take(nv) {
            for k in self.ptr[i] as usize..self.ptr[i + 1] as usize {
                if self.columns[k] as usize == i {
                    *d += self.values[k];
                }
            }
        }
        if diagonal[..nv].iter().any(|&d| d <= 0.0) {
            return Err("velocity diagonal is not positive".into());
        }
        for i in nv..n {
            for k in self.ptr[i] as usize..self.ptr[i + 1] as usize {
                let j = self.columns[k] as usize;
                if j < nv {
                    diagonal[i] += self.values[k].powi(2) / diagonal[j];
                }
            }
        }
        if diagonal.iter().any(|&d| d <= 0.0 || !d.is_finite()) {
            return Err("zero/non-finite block scaling diagonal".into());
        }
        Ok(diagonal.into_iter().map(|d| 1.0 / d.sqrt()).collect())
    }

    pub fn scaled_values(&self, scale: &[f64]) -> Vec<f32> {
        (0..self.meta.rows)
            .flat_map(|i| {
                (self.ptr[i] as usize..self.ptr[i + 1] as usize).map(move |k| {
                    (scale[i] * self.values[k] * scale[self.columns[k] as usize]) as f32
                })
            })
            .collect()
    }

    pub fn audit(&self, b: &[f64], x: &[f64], reference: &[f64], tolerance: f64) -> Audit {
        let nv = self.meta.velocity_count;
        let residual: Vec<_> = self.multiply(x).iter().zip(b).map(|(a, b)| a - b).collect();
        let difference: Vec<_> = x.iter().zip(reference).map(|(a, b)| a - b).collect();
        let relative = norm(&residual) / norm(b).max(1e-30);
        let momentum_rms = norm(&residual[..nv]) / (nv as f64).sqrt();
        let constraint_rms = norm(&residual[nv..]) / (self.meta.pressure_count as f64).sqrt();
        let momentum_relative = momentum_rms / (norm(&b[..nv]) / (nv as f64).sqrt()).max(1.0);
        let finite = x.iter().chain(&residual).all(|v| v.is_finite());
        Audit {
            relative_residual: relative,
            momentum_rms,
            momentum_relative_rms: momentum_relative,
            constraint_rms,
            velocity_reference_relative_l2: norm(&difference[..nv])
                / norm(&reference[..nv]).max(1e-30),
            pressure_reference_max_absolute: difference[nv..]
                .iter()
                .map(|v| v.abs())
                .fold(0.0, f64::max),
            finite,
            linear_residual_accepted: finite
                && relative <= tolerance
                && momentum_relative <= tolerance
                && constraint_rms <= tolerance,
        }
    }
}

#[derive(Serialize)]
pub struct Audit {
    pub relative_residual: f64,
    pub momentum_rms: f64,
    pub momentum_relative_rms: f64,
    pub constraint_rms: f64,
    pub velocity_reference_relative_l2: f64,
    pub pressure_reference_max_absolute: f64,
    pub finite: bool,
    pub linear_residual_accepted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scaling_preserves_equations_and_audit_checks_constraint_block() {
        let case = Case {
            meta: Metadata {
                format_version: 1,
                rows: 2,
                velocity_count: 1,
                pressure_count: 1,
                nnz: 3,
            },
            ptr: vec![0, 2, 3],
            columns: vec![0, 1, 0],
            values: vec![2.0, 1.0, 1.0],
            folder: PathBuf::new(),
        };
        let scale = case.scaling().unwrap();
        assert!((scale[0] - 1.0 / 2.0_f64.sqrt()).abs() < 1e-15);
        assert!((scale[1] - 2.0_f64.sqrt()).abs() < 1e-15);
        assert_eq!(case.scaled_values(&scale), vec![1.0, 1.0, 1.0]);
        let reference = [1.0, 3.0];
        let rhs = [5.0, 1.0];
        assert!(
            case.audit(&rhs, &reference, &reference, 1e-4)
                .linear_residual_accepted
        );
        let bad = case.audit(&rhs, &[1.01, 3.0], &reference, 1e-4);
        assert!(!bad.linear_residual_accepted && bad.constraint_rms > 0.009);
    }
}
