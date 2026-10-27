#[test]
fn faer_matches_custom_sparse_and_original_matrix() {
    use crate::map::MapDefinition;
    use rapier2d::prelude::Vector;
    let map = MapDefinition::small_world();
    let n = 64;
    let mask = (0..n * n)
        .map(|k| {
            map.signed_distance(Vector::new(
                -22.0 + (k % n) as f32 * 0.25 + 0.125,
                -8.0 + (k / n) as f32 * 0.25 + 0.125,
            )) < -0.08
        })
        .collect();
    let grid = Grid::new(n, 0.25, [-22.0, -8.0], mask, 1.0, [true; 4]).unwrap();
    let mut error = 0.0_f64;
    let mut residual = 0.0_f64;
    for component in &grid.components {
        let reference = super::super::custom_reference::BandCholesky::new(
            &component.diagonal,
            &component.neighbors,
        )
        .unwrap();
        for phase in [0.0, 1.3, 2.7, 5.1] {
            let rhs: Vec<_> = (0..component.diagonal.len())
                .map(|i| ((i as f64 * 0.173 + phase).sin() + (i as f64 * 0.031).cos()) * 0.2)
                .collect();
            let actual = component.factor.solve(&rhs);
            let expected = reference.solve(&rhs);
            for i in 0..rhs.len() {
                error = error.max((actual[i] - expected[i]).abs());
                let ax = component.diagonal[i] * actual[i]
                    + component.neighbors[i]
                        .iter()
                        .map(|&(j, a)| a * actual[j])
                        .sum::<f64>();
                residual = residual.max((ax - rhs[i]).abs());
            }
        }
    }
    println!("FAER_AUDIT custom_max_abs={error:.17e} matrix_residual_max_abs={residual:.17e}");
    assert!(error < 1e-10, "velocity error {error}");
    assert!(residual < 1e-10, "matrix residual {residual}");
}
