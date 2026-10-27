//! Fixed symmetric-indefinite Stokes operator, solved with pivoted sparse LU.
use faer::dyn_stack::{MemBuffer, MemStack};
use faer::sparse::linalg::lu::{self, LuRef, NumericLu, SymbolicLu};
use faer::sparse::{SparseColMat, Triplet};
use faer::{Conj, MatMut, Par};

pub(super) struct JointLu {
    symbolic: SymbolicLu<usize>,
    numeric: NumericLu<usize, f64>,
    velocity_count: usize,
}

impl JointLu {
    pub fn new(
        velocity_count: usize,
        pressure_count: usize,
        entries: &[Triplet<usize, usize, f64>],
    ) -> Result<Self, String> {
        let start = std::time::Instant::now();
        let n = velocity_count + pressure_count;
        let matrix = SparseColMat::<usize, f64>::try_new_from_triplets(n, n, entries)
            .map_err(|e| format!("joint matrix: {e:?}"))?;
        let symbolic_start = std::time::Instant::now();
        let symbolic = lu::factorize_symbolic_lu(matrix.symbolic(), Default::default())
            .map_err(|e| format!("joint symbolic: {e:?}"))?;
        let symbolic_ns = symbolic_start.elapsed().as_nanos();
        let numeric_start = std::time::Instant::now();
        let mut numeric = NumericLu::new();
        let mut scratch = MemBuffer::new(
            symbolic.factorize_numeric_lu_scratch::<f64>(Par::Seq, Default::default()),
        );
        symbolic
            .factorize_numeric_lu(
                &mut numeric,
                matrix.as_ref(),
                Par::Seq,
                MemStack::new(&mut scratch),
                Default::default(),
            )
            .map_err(|e| format!("joint numeric: {e:?}"))?;
        let numeric_ns = numeric_start.elapsed().as_nanos();
        eprintln!(
            "JOINT_FACTOR,{n},{},{symbolic_ns},{numeric_ns},{},{}",
            matrix.compute_nnz(),
            start.elapsed().as_nanos(),
            symbolic
                .solve_in_place_scratch::<f64>(1, Par::Seq)
                .unaligned_bytes_required(),
        );
        Ok(Self {
            symbolic,
            numeric,
            velocity_count,
        })
    }

    pub fn solve(&self, velocity_rhs: &[f64]) -> (Vec<f64>, Vec<f64>) {
        assert_eq!(velocity_rhs.len(), self.velocity_count);
        let mut rhs = vec![0.0; self.symbolic.nrows()];
        rhs[..self.velocity_count].copy_from_slice(velocity_rhs);
        let mut result = self.solve_full(&rhs);
        let pressure = result.split_off(self.velocity_count);
        (result, pressure)
    }

    pub fn solve_full(&self, rhs: &[f64]) -> Vec<f64> {
        let _profile = crate::profile::span(43);
        assert_eq!(rhs.len(), self.symbolic.nrows());
        let mut result = rhs.to_vec();
        let mut scratch = MemBuffer::new(self.symbolic.solve_in_place_scratch::<f64>(1, Par::Seq));
        // The matching symbolic and numeric factors are immutable after new().
        LuRef::new_unchecked(&self.symbolic, &self.numeric).solve_in_place_with_conj(
            Conj::No,
            MatMut::from_column_major_slice_mut(&mut result, rhs.len(), 1),
            Par::Seq,
            MemStack::new(&mut scratch),
        );
        result
    }
}
