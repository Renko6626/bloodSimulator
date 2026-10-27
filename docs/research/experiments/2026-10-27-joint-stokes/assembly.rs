// Appended inside fluid/grid.rs in the joint-only temporary copy.
impl Grid {
    fn build_joint(
        components: &[Component; 2],
        pressure_count: usize,
        h: f64,
    ) -> Result<super::joint::JointLu, String> {
        use faer::sparse::Triplet;
        let assembly_start = std::time::Instant::now();
        let nv: usize = components.iter().map(|c| c.faces.len()).sum();
        let mut entries = Vec::new();
        let mut offset = 0;
        for c in components {
            for (i, f) in c.faces.iter().enumerate() {
                let row = offset + i;
                entries.push(Triplet::new(row, row, c.diagonal[i]));
                for &(j, a) in &c.neighbors[i] {
                    entries.push(Triplet::new(row, offset + j, a));
                }
                // K = [ A, -B^T ; -B, 0 ], rhs = [ force ; 0 ].
                // Keep the same one-sided opening gradient as Grid::transpose.
                for (p, minus_b) in [(f.minus, -1.0 / h), (f.plus, 1.0 / h)] {
                    if p != NONE {
                        entries.push(Triplet::new(row, nv + p, minus_b));
                        entries.push(Triplet::new(nv + p, row, minus_b));
                    }
                }
            }
            offset += c.faces.len();
        }
        let result = super::joint::JointLu::new(nv, pressure_count, &entries)?;
        eprintln!("JOINT_SETUP_NS,{}", assembly_start.elapsed().as_nanos());
        Ok(result)
    }
}
