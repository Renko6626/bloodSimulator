use crate::grid::{Config, Grid};

#[test]
fn projection_removes_actual_mac_divergence() {
    for periodic_y in [false, true] {
        let mut g = Grid::new(Config::new(18, 12, 6.0, 4.0, 0.01, 0.1, periodic_y));
        g.set_velocity(|x, y| [(x * 1.7 + y).sin(), (x - y * 2.3).cos()]);
        assert!(g.diagnostics().0 > 0.1);
        g.project();
        assert!(g.diagnostics().0 < 1e-10, "{:?}", g.diagnostics());
    }
}

#[test]
fn implicit_viscosity_dissipates_energy() {
    let mut g = Grid::new(Config::new(18, 12, 6.0, 4.0, 0.01, 0.1, false));
    g.set_velocity(|x, y| [x.sin() * y.sin(), (x * y).cos()]);
    let before = g.diagnostics().1;
    g.diffuse();
    assert!(g.diagnostics().1 < before);
}

#[test]
fn steady_channel_preserves_analytic_flow_with_second_order_wall_error() {
    let mut errors = Vec::new();
    for ny in [12, 24] {
        let h = 4.0;
        let nu = 0.3;
        let force = 0.7;
        let dt = 0.02;
        let mut g = Grid::new(Config::new(8, ny, 6.0, h, dt, nu, false));
        g.set_velocity(|_, y| [force * y * (h - y) / (2.0 * nu), 0.0]);
        for _ in 0..50 {
            g.step(force);
        }
        errors.push(g.relative_error(|_, y| [force * y * (h - y) / (2.0 * nu), 0.0]));
    }
    assert!(errors[1] < errors[0] * 0.35, "{errors:?}");
    assert!(errors[1] < 0.002, "{errors:?}");
}

#[test]
fn drag_exchanges_equal_opposite_momentum() {
    use crate::coupling::Cells;
    let mut g = Grid::new(Config::new(32, 20, 48.0, 7.5, 1.0 / 120.0, 0.75, false));
    g.set_velocity(|_, _| [10.0, 0.3]);
    let mut cells = Cells::new(&g, 24);
    let mass = g.c.hx() * g.c.hy();
    let momentum = |g: &Grid| {
        [
            g.u.iter().sum::<f64>() * mass,
            g.v.iter().sum::<f64>() * mass,
        ]
    };
    let before = momentum(&g);
    let mismatch = cells.exchange(&mut g);
    let after = momentum(&g);
    let body = cells.momentum();
    assert!(mismatch < 1e-8, "{mismatch}");
    for c in 0..2 {
        assert!(
            (after[c] - before[c] + body[c]).abs() < 1e-7,
            "{before:?} {after:?} {body:?}"
        );
    }
    assert!(body[0] > 0.0);
}
