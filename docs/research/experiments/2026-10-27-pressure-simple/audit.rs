// Same P action formed independently as B diag(A)^-1 B^T, including openings.
#[test]
fn simple_factor_inverts_original_approximate_schur() {
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
    let mut max_residual = 0.0_f64;
    for phase in [0.0, 1.3] {
        let rhs: Vec<_> = (0..grid.cells.len())
            .map(|i| ((i as f64 * 0.173 + phase).sin() + (i as f64 * 0.031).cos()) * 0.2)
            .collect();
        let z = grid.precondition(&rhs);
        assert!(dot(&rhs, &z) > 0.0);
        let mut diagonal_velocity = grid.transpose(&z);
        for (v, a) in diagonal_velocity
            .iter_mut()
            .zip(grid.components.iter().flat_map(|c| &c.diagonal))
        {
            *v /= a;
        }
        let pz = grid.divergence(&diagonal_velocity);
        for (a, b) in pz.iter().zip(&rhs) {
            max_residual = max_residual.max((a - b).abs());
        }
    }
    println!("SIMPLE_AUDIT max_abs_Pz_minus_rhs={max_residual:.17e}");
    assert!(max_residual < 1e-10, "P solve residual={max_residual}");
}
