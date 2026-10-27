//! Research-only SPD solver: greedy minimum degree, symbolic fill, and sparse
//! left-looking Cholesky. Keeps the baseline interface inside a copied crate.
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap};

pub(super) struct BandCholesky {
    order: Vec<usize>,
    columns: Vec<Vec<usize>>,
    values: Vec<Vec<f64>>,
    diagonal: Vec<f64>,
}

impl BandCholesky {
    pub fn new(diagonal: &[f64], neighbors: &[Vec<(usize, f64)>]) -> Result<Self, String> {
        let size = diagonal.len();
        let mut graph: Vec<BTreeSet<usize>> = neighbors
            .iter()
            .map(|ns| ns.iter().map(|&(j, _)| j).collect())
            .collect();
        let mut heap: BinaryHeap<_> = graph
            .iter()
            .enumerate()
            .map(|(i, ns)| Reverse((ns.len(), i)))
            .collect();
        let mut active = vec![true; size];
        let mut order = Vec::with_capacity(size);
        let mut patterns = Vec::with_capacity(size);
        while let Some(Reverse((degree, node))) = heap.pop() {
            if !active[node] || degree != graph[node].len() {
                continue;
            }
            let ns: Vec<_> = graph[node].iter().copied().collect();
            for &i in &ns {
                graph[i].remove(&node);
            }
            for (offset, &i) in ns.iter().enumerate() {
                for &j in &ns[offset + 1..] {
                    graph[i].insert(j);
                    graph[j].insert(i);
                }
            }
            for &i in &ns {
                heap.push(Reverse((graph[i].len(), i)));
            }
            active[node] = false;
            graph[node].clear();
            order.push(node);
            patterns.push(ns);
        }
        assert_eq!(order.len(), size);
        let mut inverse = vec![0; size];
        for (new, &old) in order.iter().enumerate() {
            inverse[old] = new;
        }
        let columns: Vec<Vec<usize>> = patterns
            .into_iter()
            .map(|ns| {
                let mut rows: Vec<_> = ns.into_iter().map(|i| inverse[i]).collect();
                rows.sort_unstable();
                rows
            })
            .collect();
        let mut row_links = vec![Vec::new(); size];
        for (k, rows) in columns.iter().enumerate() {
            for (offset, &i) in rows.iter().enumerate() {
                row_links[i].push((k, offset));
            }
        }
        let mut values: Vec<Vec<f64>> = columns.iter().map(|c| vec![0.0; c.len()]).collect();
        let mut roots = vec![0.0; size];
        let mut workspace = vec![0.0; size];
        for j in 0..size {
            workspace.fill(0.0);
            workspace[j] = diagonal[order[j]];
            for &(old, a) in &neighbors[order[j]] {
                if inverse[old] > j {
                    workspace[inverse[old]] = a;
                }
            }
            for &(k, offset) in &row_links[j] {
                let ljk = values[k][offset];
                workspace[j] -= ljk * ljk;
                for m in offset + 1..columns[k].len() {
                    workspace[columns[k][m]] -= values[k][m] * ljk;
                }
            }
            if !workspace[j].is_finite() || workspace[j] <= 0.0 {
                return Err(format!(
                    "sparse velocity matrix is not SPD at {j}: {}",
                    workspace[j]
                ));
            }
            roots[j] = workspace[j].sqrt();
            for (offset, &i) in columns[j].iter().enumerate() {
                values[j][offset] = workspace[i] / roots[j];
            }
        }
        Ok(Self {
            order,
            columns,
            values,
            diagonal: roots,
        })
    }

    pub fn solve(&self, rhs: &[f64]) -> Vec<f64> {
        assert_eq!(rhs.len(), self.order.len());
        let mut x: Vec<_> = self.order.iter().map(|&old| rhs[old]).collect();
        for j in 0..x.len() {
            x[j] /= self.diagonal[j];
            let solved = x[j];
            for (&i, &a) in self.columns[j].iter().zip(&self.values[j]) {
                x[i] -= a * solved;
            }
        }
        for j in (0..x.len()).rev() {
            let mut value = x[j];
            for (&i, &a) in self.columns[j].iter().zip(&self.values[j]) {
                value -= a * x[i];
            }
            x[j] = value / self.diagonal[j];
        }
        let mut result = vec![0.0; x.len()];
        for (new, &old) in self.order.iter().enumerate() {
            result[old] = x[new];
        }
        result
    }

    pub fn factor_stats(&self) -> (usize, usize) {
        let off_diagonal: usize = self.columns.iter().map(Vec::len).sum();
        let n = self.order.len();
        // Array payload only: excludes Vec headers/capacity and temporary setup.
        (n + off_diagonal, (n + off_diagonal) * 16)
    }
}
