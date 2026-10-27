//! Fixed SPD matrix with persistent numeric factors and solve scratch.
use faer::dyn_stack::{MemBuffer, MemStack};
use faer::sparse::linalg::cholesky::{self, LltRef, SymbolicCholesky};
use faer::sparse::{SparseColMat, Triplet};
use faer::{Conj, MatMut, Par, Side};

pub struct Factor {
    symbolic: SymbolicCholesky<usize>,
    numeric: Vec<f64>,
    scratch: MemBuffer,
}
impl Factor {
    pub fn new(n: usize, entries: &[Triplet<usize, usize, f64>]) -> Self {
        let matrix = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, entries).unwrap();
        let symbolic = cholesky::factorize_symbolic_cholesky(
            matrix.symbolic(),
            Side::Lower,
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let mut numeric = vec![0.0; symbolic.len_val()];
        let mut scratch = MemBuffer::new(
            symbolic.factorize_numeric_llt_scratch::<f64>(Par::Seq, Default::default()),
        );
        symbolic
            .factorize_numeric_llt(
                &mut numeric,
                matrix.as_ref(),
                Side::Lower,
                Default::default(),
                Par::Seq,
                MemStack::new(&mut scratch),
                Default::default(),
            )
            .unwrap();
        let scratch = MemBuffer::new(symbolic.solve_in_place_scratch::<f64>(1, Par::Seq));
        Self {
            symbolic,
            numeric,
            scratch,
        }
    }
    pub fn solve(&mut self, rhs: &mut [f64]) {
        LltRef::new(&self.symbolic, &self.numeric).solve_in_place_with_conj(
            Conj::No,
            MatMut::from_column_major_slice_mut(rhs, self.symbolic.nrows(), 1),
            Par::Seq,
            MemStack::new(&mut self.scratch),
        );
    }
    pub fn numeric_bytes(&self) -> usize {
        self.numeric.len() * 8
    }
}
