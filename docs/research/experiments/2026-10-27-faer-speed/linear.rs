//! Research adapter: faer sparse LLT, default AMD ordering and automatic kernel.
//! Keep the existing outer bounded Rayon join; each component runs sequentially.
use faer::dyn_stack::{MemBuffer, MemStack};
use faer::sparse::linalg::cholesky::{self, LltRef, SymbolicCholesky};
use faer::sparse::{SparseColMat, Triplet};
use faer::{Conj, MatMut, Par, Side};

pub(super) struct BandCholesky {
    symbolic: SymbolicCholesky<usize>,
    numeric: Vec<f64>,
}

impl BandCholesky {
    pub fn new(diagonal: &[f64], neighbors: &[Vec<(usize, f64)>]) -> Result<Self, String> {
        let n = diagonal.len();
        let mut triplets = Vec::new();
        for i in 0..n {
            triplets.push(Triplet::new(i, i, diagonal[i]));
            for &(j, value) in &neighbors[i] {
                if i > j {
                    triplets.push(Triplet::new(i, j, value));
                }
            }
        }
        let matrix = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, &triplets)
            .map_err(|e| format!("faer matrix: {e:?}"))?;
        let symbolic = cholesky::factorize_symbolic_cholesky(
            matrix.symbolic(),
            Side::Lower,
            Default::default(),
            Default::default(),
        )
        .map_err(|e| format!("faer symbolic: {e:?}"))?;
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
            .map_err(|e| format!("faer numeric: {e:?}"))?;
        eprintln!(
            "FAER_KERNEL,{:?},numeric_entries={},solve_scratch_bytes={}",
            match symbolic.raw() {
                cholesky::SymbolicCholeskyRaw::Simplicial(_) => "simplicial",
                cholesky::SymbolicCholeskyRaw::Supernodal(_) => "supernodal",
            },
            numeric.len(),
            symbolic
                .solve_in_place_scratch::<f64>(1, Par::Seq)
                .unaligned_bytes_required(),
        );
        Ok(Self { symbolic, numeric })
    }

    pub fn solve(&self, rhs: &[f64]) -> Vec<f64> {
        let _profile = crate::profile::span(22);
        assert_eq!(rhs.len(), self.symbolic.nrows());
        let mut result = rhs.to_vec();
        let mut scratch = MemBuffer::new(self.symbolic.solve_in_place_scratch::<f64>(1, Par::Seq));
        LltRef::new(&self.symbolic, &self.numeric).solve_in_place_with_conj(
            Conj::No,
            MatMut::from_column_major_slice_mut(&mut result, rhs.len(), 1),
            Par::Seq,
            MemStack::new(&mut scratch),
        );
        result
    }

    pub fn factor_stats(&self) -> (usize, usize) {
        // Numeric values ONLY. Opaque symbolic metadata and scratch excluded;
        // not the same scope as the custom solver's array-payload byte count.
        (self.numeric.len(), self.numeric.len() * size_of::<f64>())
    }
}
