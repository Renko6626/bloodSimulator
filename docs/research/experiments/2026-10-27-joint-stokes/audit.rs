#[test]
fn joint_matches_original_block_operator_and_strict_cg() {
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
    let nv = grid.velocity_count();
    let np = grid.cells.len();
    let mut max_roundtrip = 0.0_f64;
    let mut max_velocity_difference = 0.0_f64;
    let mut max_pressure_difference = 0.0_f64;
    let mut max_divergence = 0.0_f64;
    let mut max_momentum = 0.0_f64;
    for phase in [0.0, 1.3] {
        let exact_u: Vec<_> = (0..nv)
            .map(|i| (i as f64 * 0.173 + phase).sin() * 0.2)
            .collect();
        let exact_p: Vec<_> = (0..np)
            .map(|i| (i as f64 * 0.031 + phase).cos() * 0.2)
            .collect();
        let gradient = grid.transpose(&exact_p);
        let mut rhs = Vec::new();
        let mut offset = 0;
        for c in &grid.components {
            for i in 0..c.faces.len() {
                let au = c.diagonal[i] * exact_u[offset + i]
                    + c.neighbors[i]
                        .iter()
                        .map(|&(j, a)| a * exact_u[offset + j])
                        .sum::<f64>();
                rhs.push(au - gradient[offset + i]);
            }
            offset += c.faces.len();
        }
        rhs.extend(grid.divergence(&exact_u).into_iter().map(|d| -d));
        let actual = grid.joint.solve_full(&rhs);
        for (x, y) in actual.iter().zip(exact_u.iter().chain(&exact_p)) {
            max_roundtrip = max_roundtrip.max((x - y).abs());
        }
        // Nonzero forcing with real homogeneous traction openings; independent
        // reference is the retained original strict Schur-CG, not reassembly K.
        let force: Vec<_> = (0..nv)
            .map(|i| ((i as f64 * 0.173 + phase).sin() + (i as f64 * 0.031).cos()) * 0.2)
            .collect();
        let direct = grid.solve(&force, &|_| 0.0, None).unwrap();
        let reference = grid.solve_cg_reference(&force, &|_| 0.0, None).unwrap();
        for (a, b) in direct.velocity.iter().zip(&reference.velocity) {
            max_velocity_difference = max_velocity_difference.max((a - b).abs());
        }
        for (a, b) in direct.pressure.iter().zip(&reference.pressure) {
            max_pressure_difference = max_pressure_difference.max((a - b).abs());
        }
        max_divergence = max_divergence.max(direct.divergence_rms);
        max_momentum = max_momentum.max(direct.momentum_rms);
    }
    println!(
        "JOINT_AUDIT roundtrip_max_abs={max_roundtrip:.17e} cg_velocity_max_abs={max_velocity_difference:.17e} cg_pressure_max_abs={max_pressure_difference:.17e} divergence_rms_max={max_divergence:.17e} momentum_rms_max={max_momentum:.17e}"
    );
    assert!(
        max_roundtrip < 1e-9,
        "block operator roundtrip={max_roundtrip}"
    );
    assert!(
        max_velocity_difference < 1e-6,
        "CG velocity difference={max_velocity_difference}"
    );
    assert!(
        max_pressure_difference < 1e-5,
        "CG pressure difference={max_pressure_difference}"
    );
    assert!(max_divergence < 1e-10);
    assert!(max_momentum < 1e-10);
}
