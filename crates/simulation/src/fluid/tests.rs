use super::grid::Grid;

#[test]
fn open_channel_matches_poiseuille_and_preserves_pressure_reference() {
    let n = 16;
    let h = 1.0 / n as f64;
    let mut grid = Grid::new(
        n,
        h,
        [0.0, 0.0],
        vec![true; n * n],
        1.0,
        [true, true, false, false],
    )
    .unwrap();
    let boundary = |p: [f64; 2]| 1.0 - p[0];
    let field = grid
        .solve(&vec![0.0; grid.velocity_count()], &boundary)
        .unwrap();
    let shifted = grid
        .solve(&vec![0.0; grid.velocity_count()], &|p| boundary(p) + 7.0)
        .unwrap();
    let mean: f64 = (0..n)
        .map(|j| grid.sample(&field, [0.5, (j as f64 + 0.5) * h])[0])
        .sum::<f64>()
        / n as f64;
    assert!((mean / (1.0 / 12.0) - 1.0).abs() < 0.015, "mean={mean}");
    assert!(field.divergence_rms < 1e-8, "div={}", field.divergence_rms);
    assert!(field.momentum_rms < 1e-8, "momentum={}", field.momentum_rms);
    for (a, b) in field.velocity.iter().zip(&shifted.velocity) {
        assert!((a - b).abs() < 1e-8);
    }
    for (a, b) in field.pressure.iter().zip(&shifted.pressure) {
        assert!((b - a - 7.0).abs() < 1e-7);
    }
    assert!(field.net_flux.abs() < 1e-8);
}

#[test]
fn equal_open_pressures_rest_and_closed_component_is_rejected() {
    let mut grid = Grid::new(
        8,
        0.125,
        [0.0, 0.0],
        vec![true; 64],
        1.0,
        [true, true, false, false],
    )
    .unwrap();
    let f = grid
        .solve(&vec![0.0; grid.velocity_count()], &|_| 3.0)
        .unwrap();
    assert!(f.velocity.iter().all(|v| v.abs() < 1e-9));
    assert!(Grid::new(8, 0.125, [0.0, 0.0], vec![true; 64], 1.0, [false; 4]).is_err());
}

#[test]
fn real_fork_conserves_opening_flux_and_solves_true_stokes_residuals() {
    use crate::{circulation::graph::GraphNetwork, map::MapDefinition};
    use rapier2d::prelude::Vector;
    use std::time::Instant;
    let map = MapDefinition::small_world();
    let flow = GraphNetwork::new(&map, 0.05).solve(1.0);
    let n = 64;
    let mask = (0..n * n)
        .map(|k| {
            map.signed_distance(Vector::new(
                -22.0 + (k % n) as f32 * 0.25 + 0.125,
                -8.0 + (k / n) as f32 * 0.25 + 0.125,
            )) < -0.08
        })
        .collect();
    let start = Instant::now();
    let mut grid = Grid::new(n, 0.25, [-22.0, -8.0], mask, 1.0, [true; 4]).unwrap();
    let init = start.elapsed();
    let boundary = |p: [f64; 2]| {
        let p = Vector::new(p[0] as f32, p[1] as f32);
        let edge = map.nearest_edge(p);
        let projection = map.project_edge(edge, p);
        let t = (projection.along / map.edge_length(edge)).clamp(0.0, 1.0) as f64;
        let e = &map.edges[edge];
        7.5 * ((1.0 - t) * flow.pressures[e.from] as f64 + t * flow.pressures[e.to] as f64)
    };
    let start = Instant::now();
    let field = grid
        .solve(&vec![0.0; grid.velocity_count()], &boundary)
        .unwrap();
    println!(
        "fork init={init:?} solve={:?} CG={} div={} momentum={} net={}",
        start.elapsed(),
        field.iterations,
        field.divergence_rms,
        field.momentum_rms,
        field.net_flux
    );
    assert!(field.divergence_rms < 1e-8);
    assert!(field.momentum_rms < 1e-8);
    assert!(field.net_flux.abs() < 1e-7);
    assert!(grid.sample(&field, [-18.0, 0.0])[0] > 0.0);
}
