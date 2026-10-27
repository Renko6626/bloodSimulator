// Appended inside fluid/grid.rs in the SIMPLE-only temporary copy.
impl Grid {
    fn build_pressure_preconditioner(
        components: &[Component; 2],
        count: usize,
        h: f64,
    ) -> Result<super::pressure_linear::BandCholesky, String> {
        let start = std::time::Instant::now();
        let mut diagonal = vec![0.0; count];
        let mut neighbors = vec![Vec::new(); count];
        for c in components {
            for (f, &a) in c.faces.iter().zip(&c.diagonal) {
                let weight = 1.0 / (h * h * a);
                if f.minus != NONE {
                    diagonal[f.minus] += weight;
                }
                if f.plus != NONE {
                    diagonal[f.plus] += weight;
                }
                if f.minus != NONE && f.plus != NONE {
                    neighbors[f.minus].push((f.plus, -weight));
                    neighbors[f.plus].push((f.minus, -weight));
                }
            }
        }
        let factor = super::pressure_linear::BandCholesky::new(&diagonal, &neighbors)?;
        let (entries, payload) = factor.factor_stats();
        eprintln!(
            "PRESSURE_FACTOR,{count},{entries},{payload},{}",
            start.elapsed().as_nanos()
        );
        Ok(factor)
    }

    fn precondition(&self, residual: &[f64]) -> Vec<f64> {
        let _profile = crate::profile::span(43);
        self.pressure_preconditioner.solve(residual)
    }
}
