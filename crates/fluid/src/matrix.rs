use crate::FluidResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CsrMatrix {
    pub rows: usize,
    pub cols: usize,
    pub ptr: Vec<u32>,
    pub columns: Vec<u32>,
    pub values: Vec<f64>,
}
impl CsrMatrix {
    pub fn from_triplets(rows: usize, entries: &[(usize, usize, f64)]) -> FluidResult<Self> {
        Self::from_rect_triplets(rows, rows, entries)
    }
    pub fn from_rect_triplets(
        rows: usize,
        cols: usize,
        entries: &[(usize, usize, f64)],
    ) -> FluidResult<Self> {
        if rows == 0 || cols == 0 || rows > u32::MAX as usize {
            return Err("invalid matrix order".into());
        }
        let mut data = vec![BTreeMap::<usize, f64>::new(); rows];
        for &(i, j, a) in entries {
            if i >= rows || j >= cols || !a.is_finite() {
                return Err("invalid matrix entry".into());
            }
            *data[i].entry(j).or_default() += a;
        }
        let mut matrix = Self {
            rows,
            cols,
            ptr: vec![0],
            columns: Vec::new(),
            values: Vec::new(),
        };
        for row in data {
            for (j, a) in row {
                if a != 0.0 {
                    matrix.columns.push(j as u32);
                    matrix.values.push(a);
                }
            }
            matrix.ptr.push(matrix.columns.len() as u32);
        }
        Ok(matrix)
    }
    pub fn validate(&self) -> FluidResult<()> {
        if self.rows == 0
            || self.ptr.len() != self.rows + 1
            || self.ptr.first() != Some(&0)
            || self.ptr.last().copied() != Some(self.values.len() as u32)
            || self.columns.len() != self.values.len()
            || self.ptr.windows(2).any(|p| p[0] > p[1])
            || self.columns.iter().any(|&j| j as usize >= self.cols)
            || self.values.iter().any(|v| !v.is_finite())
        {
            return Err("invalid CSR data".into());
        }
        Ok(())
    }
    pub fn row(&self, i: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        (self.ptr[i] as usize..self.ptr[i + 1] as usize)
            .map(|k| (self.columns[k] as usize, self.values[k]))
    }
    pub fn apply(&self, x: &[f64]) -> Vec<f64> {
        (0..self.rows)
            .map(|i| self.row(i).map(|(j, a)| a * x[j]).sum())
            .collect()
    }
    pub fn diagonal(&self) -> Vec<f64> {
        (0..self.rows)
            .map(|i| self.row(i).filter(|(j, _)| *j == i).map(|(_, a)| a).sum())
            .collect()
    }
    pub fn leading(&self, n: usize) -> FluidResult<Self> {
        let entries: Vec<_> = (0..n)
            .flat_map(|i| {
                self.row(i)
                    .filter(move |(j, _)| *j < n)
                    .map(move |(j, a)| (i, j, a))
            })
            .collect();
        Self::from_triplets(n, &entries)
    }
    pub fn scaled(&self, d: &[f64]) -> Self {
        let mut out = self.clone();
        for i in 0..self.rows {
            for k in self.ptr[i] as usize..self.ptr[i + 1] as usize {
                out.values[k] *= d[i] * d[self.columns[k] as usize];
            }
        }
        out
    }
}
pub fn norm(x: &[f64]) -> f64 {
    x.iter().map(|x| x * x).sum::<f64>().sqrt()
}
pub fn dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(x, y)| x * y).sum()
}
