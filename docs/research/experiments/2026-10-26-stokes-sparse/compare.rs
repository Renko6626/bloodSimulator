/// Same assembled fork A, nontrivial RHSs: compare both exact factorizations
/// and independently form A*x-b using the original matrix, without pressure CG.
#[test]
fn fork_speed_inverse_matches_band_and_original_matrix() {
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
    let mut max_error = 0.0_f64;
    let mut max_residual = 0.0_f64;
    for component in &grid.components {
        let reference =
            super::super::reference::BandCholesky::new(&component.diagonal, &component.neighbors)
                .unwrap();
        for phase in [0.0, 1.3] {
            let rhs: Vec<_> = (0..component.diagonal.len())
                .map(|i| ((i as f64 * 0.173 + phase).sin() + (i as f64 * 0.031).cos()) * 0.2)
                .collect();
            let actual = component.factor.solve(&rhs);
            let expected = reference.solve(&rhs);
            for i in 0..rhs.len() {
                max_error = max_error.max((actual[i] - expected[i]).abs());
                let ax = component.diagonal[i] * actual[i]
                    + component.neighbors[i]
                        .iter()
                        .map(|&(j, a)| a * actual[j])
                        .sum::<f64>();
                max_residual = max_residual.max((ax - rhs[i]).abs());
            }
        }
    }
    println!(
        "COMPARE velocity_max_abs={max_error:.17e} matrix_residual_max_abs={max_residual:.17e}"
    );
    assert!(max_error < 1e-10, "velocity error {max_error}");
    assert!(max_residual < 1e-10, "matrix residual {max_residual}");
}
