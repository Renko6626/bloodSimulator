//! Connectivity-aware smoothed aggregation; Galerkin transfer and fixed V-cycles.
#[cfg(test)]
use crate::matrix::norm;
use crate::{CsrMatrix, FluidResult};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Level {
    pub matrix: CsrMatrix,
    pub inverse_diagonal: Vec<f64>,
    pub omega: f64,
    pub prolongation: Option<CsrMatrix>,
    pub restriction: Option<CsrMatrix>,
}
#[derive(Clone, Debug)]
pub struct Hierarchy {
    pub levels: Vec<Level>,
    pub coarse_inverse: CsrMatrix,
    pub smooth_steps: usize,
}

fn inverse(a: &CsrMatrix) -> FluidResult<CsrMatrix> {
    let n = a.rows;
    let mut l = vec![0.; n * n];
    for i in 0..n {
        for (j, v) in a.row(i) {
            l[i * n + j] = v;
        }
    }
    for i in 0..n {
        for j in 0..=i {
            let value = l[i * n + j] - (0..j).map(|k| l[i * n + k] * l[j * n + k]).sum::<f64>();
            if i == j {
                if value <= 0. || !value.is_finite() {
                    return Err("MG coarse matrix is not SPD".into());
                }
                l[i * n + j] = value.sqrt();
            } else {
                l[i * n + j] = value / l[j * n + j];
            }
        }
    }
    let mut entries = Vec::new();
    for j in 0..n {
        let mut y = vec![0.; n];
        for i in 0..n {
            y[i] = (f64::from(i == j) - (0..i).map(|k| l[i * n + k] * y[k]).sum::<f64>())
                / l[i * n + i];
        }
        let mut x = vec![0.; n];
        for i in (0..n).rev() {
            x[i] = (y[i] - (i + 1..n).map(|k| l[k * n + i] * x[k]).sum::<f64>()) / l[i * n + i];
        }
        for (i, v) in x.into_iter().enumerate() {
            entries.push((i, j, v));
        }
    }
    CsrMatrix::from_triplets(n, &entries)
}

impl Hierarchy {
    pub fn build(
        mut a: CsrMatrix,
        mut positions: Option<Vec<[f64; 2]>>,
        mut components: Vec<u8>,
        mut width: f64,
        coarse_limit: usize,
    ) -> FluidResult<Self> {
        let mut levels = Vec::new();
        loop {
            let diag = a.diagonal();
            if diag.iter().any(|&v| v <= 0. || !v.is_finite()) {
                return Err("non-positive MG diagonal".into());
            }
            let bound = (0..a.rows)
                .map(|i| a.row(i).map(|(_, v)| v.abs()).sum::<f64>() / diag[i])
                .fold(1., f64::max);
            let omega = 4. / (3. * bound);
            let mut level = Level {
                matrix: a.clone(),
                inverse_diagonal: diag.iter().map(|v| 1. / v).collect(),
                omega,
                prolongation: None,
                restriction: None,
            };
            if a.rows <= coarse_limit {
                let coarse_inverse = inverse(&a)?;
                levels.push(level);
                return Ok(Self {
                    levels,
                    coarse_inverse,
                    smooth_steps: 3,
                });
            }
            if levels.len() >= 16 {
                return Err("MG coarsening did not reach the bounded coarse solve".into());
            }
            let tiles: Vec<_> = (0..a.rows)
                .map(|i| {
                    positions.as_ref().map(|p| {
                        [
                            (p[i][0] / width).floor() as i64,
                            (p[i][1] / width).floor() as i64,
                        ]
                    })
                })
                .collect();
            let mut aggregate = vec![usize::MAX; a.rows];
            let mut groups = Vec::<Vec<usize>>::new();
            for seed in 0..a.rows {
                if aggregate[seed] != usize::MAX {
                    continue;
                }
                let id = groups.len();
                aggregate[seed] = id;
                let mut members = vec![seed];
                let mut cursor = 0;
                while cursor < members.len() {
                    let i = members[cursor];
                    cursor += 1;
                    for (j, v) in a.row(i) {
                        if j != i
                            && v != 0.
                            && aggregate[j] == usize::MAX
                            && components[j] == components[seed]
                            && tiles[j] == tiles[seed]
                            && (positions.is_some() || members.len() < 4)
                        {
                            aggregate[j] = id;
                            members.push(j);
                        }
                    }
                }
                groups.push(members);
            }
            if groups.len() == a.rows {
                // Spatial tiles may initially separate connected boundary faces.
                // Increase tile width; disconnected singleton systems are diagonal.
                if a.values.len() == a.rows {
                    let coarse_inverse = CsrMatrix::from_triplets(
                        a.rows,
                        &diag
                            .iter()
                            .enumerate()
                            .map(|(i, d)| (i, i, 1. / d))
                            .collect::<Vec<_>>(),
                    )?;
                    levels.push(level);
                    return Ok(Self {
                        levels,
                        coarse_inverse,
                        smooth_steps: 3,
                    });
                }
                width *= 2.;
                continue;
            }
            let nc = groups.len();
            let mut pe = Vec::new();
            let mut p_rows = Vec::new();
            for i in 0..a.rows {
                let mut row = BTreeMap::from([(aggregate[i], 1.)]);
                for (j, v) in a.row(i) {
                    *row.entry(aggregate[j]).or_default() -= omega * v / diag[i];
                }
                row.retain(|_, v| *v != 0.);
                for (&j, &v) in &row {
                    pe.push((i, j, v));
                }
                p_rows.push(row);
            }
            let re: Vec<_> = pe.iter().map(|&(i, j, v)| (j, i, v)).collect();
            let mut ce = Vec::new();
            for i in 0..a.rows {
                for (j, v) in a.row(i) {
                    for (&g, &p) in &p_rows[i] {
                        for (&h, &q) in &p_rows[j] {
                            ce.push((g, h, p * v * q));
                        }
                    }
                }
            }
            let next = CsrMatrix::from_triplets(nc, &ce)?;
            level.prolongation = Some(CsrMatrix::from_rect_triplets(a.rows, nc, &pe)?);
            level.restriction = Some(CsrMatrix::from_rect_triplets(nc, a.rows, &re)?);
            levels.push(level);
            positions = positions.as_ref().map(|p| {
                groups
                    .iter()
                    .map(|g| {
                        [
                            g.iter().map(|&i| p[i][0]).sum::<f64>() / g.len() as f64,
                            g.iter().map(|&i| p[i][1]).sum::<f64>() / g.len() as f64,
                        ]
                    })
                    .collect()
            });
            components = groups.iter().map(|g| components[g[0]]).collect();
            a = next;
            width *= 2.;
        }
    }
    fn cycle(&self, k: usize, b: &[f64]) -> Vec<f64> {
        let l = &self.levels[k];
        if k + 1 == self.levels.len() {
            return self.coarse_inverse.apply(b);
        }
        let smooth = |x: &mut Vec<f64>| {
            for _ in 0..self.smooth_steps {
                let ax = l.matrix.apply(x);
                for i in 0..x.len() {
                    x[i] += l.omega * l.inverse_diagonal[i] * (b[i] - ax[i]);
                }
            }
        };
        let mut x = vec![0.; b.len()];
        smooth(&mut x);
        let r: Vec<_> = b
            .iter()
            .zip(l.matrix.apply(&x))
            .map(|(b, a)| b - a)
            .collect();
        let coarse_b = l.restriction.as_ref().unwrap().apply(&r);
        let correction = l
            .prolongation
            .as_ref()
            .unwrap()
            .apply(&self.cycle(k + 1, &coarse_b));
        for (x, c) in x.iter_mut().zip(correction) {
            *x += c;
        }
        smooth(&mut x);
        x
    }
    pub fn apply(&self, b: &[f64]) -> Vec<f64> {
        self.apply_cycles(b, 2)
    }
    pub fn apply_cycles(&self, b: &[f64], cycles: usize) -> Vec<f64> {
        let mut x = vec![0.; b.len()];
        for _ in 0..cycles {
            let r: Vec<_> = b
                .iter()
                .zip(self.levels[0].matrix.apply(&x))
                .map(|(b, a)| b - a)
                .collect();
            let correction = self.cycle(0, &r);
            for (x, c) in x.iter_mut().zip(correction) {
                *x += c;
            }
        }
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aggregation_does_not_join_disconnected_nearby_branches() {
        let mut e: Vec<_> = (0..8).map(|i| (i, i, 3.)).collect();
        for (i, j) in [(0, 2), (2, 4), (4, 6), (1, 3), (3, 5), (5, 7)] {
            e.push((i, j, -1.));
            e.push((j, i, -1.));
        }
        let a = CsrMatrix::from_triplets(8, &e).unwrap();
        let h = Hierarchy::build(
            a.clone(),
            Some(
                (0..8)
                    .map(|i| [(i / 2) as f64 * 0.1, (i % 2) as f64 * 0.01])
                    .collect(),
            ),
            vec![0; 8],
            2.,
            2,
        )
        .unwrap();
        let p = h.levels[0].prolongation.as_ref().unwrap();
        assert_eq!(p.cols, 2);
        let even: Vec<_> = p.row(0).map(|(j, _)| j).collect();
        let odd: Vec<_> = p.row(1).map(|(j, _)| j).collect();
        assert!(even.iter().all(|j| !odd.contains(j)));
        let b = vec![1.; 8];
        let x = h.apply(&b);
        let residual: Vec<_> = b.iter().zip(a.apply(&x)).map(|(b, a)| b - a).collect();
        assert!(norm(&residual) < norm(&b));
    }
}
