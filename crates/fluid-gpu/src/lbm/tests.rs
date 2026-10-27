use super::*;

#[test]
fn lbm_uniform_equilibrium_and_coordinates() {
    let fine = WindowSpec {
        origin: WindowOrigin {
            block_x: -1,
            block_y: 2,
        },
        profile: GridProfile::Fine,
    };
    let coarse = WindowSpec {
        profile: GridProfile::Coarse,
        ..fine
    };
    let a = model::cell_position(fine, 2, 2);
    let b = model::cell_position(coarse, 1, 1);
    assert!((a[0] - b[0]).abs() <= 0.125 && (a[1] - b[1]).abs() <= 0.125);
    let g = model::equilibrium(1.0, [0.0, 0.0]);
    assert!(g.iter().all(|x| x.abs() < 1e-7));
    assert!((1.0 + g.iter().sum::<f32>() - 1.0).abs() < 1e-7);
}

#[test]
#[ignore = "explicit software-adapter functional diagnostic"]
fn lbm_software_window_smoke() {
    let spec = WindowSpec {
        origin: WindowOrigin {
            block_x: 0,
            block_y: 0,
        },
        profile: GridProfile::Coarse,
    };
    let n = spec.profile.cells_per_side() as usize;
    let mut density = vec![1.0; n * n];
    density[40 * n + 40] = 1.05;
    let initial = InitialField {
        density,
        velocity: vec![[0.0, 0.0]; n * n],
    };
    let geometry = WindowGeometry {
        solid: vec![false; n * n],
        openings: vec![],
    };
    let config = WindowConfig {
        dt: 0.01,
        viscosity: 0.1,
        allow_software: true,
    };
    let mut window = LbmWindow::new(spec, config, initial, geometry.clone()).unwrap();
    window
        .advance(StepInput {
            acceleration: [0.0, 0.0],
        })
        .unwrap();
    let stationary = window.sample([30.25, 20.25]).unwrap();
    assert!((stationary.density - 1.0).abs() < 1e-6);
    assert!(stationary.velocity[0].abs() < 1e-6 && stationary.velocity[1].abs() < 1e-6);
    let before = window.sample([20.25, 20.25]).unwrap();
    assert!(before.density > 1.0);
    let original_populations = window.populations_for_test();
    let strip = InitialField {
        density: vec![1.0; n * n],
        velocity: vec![[0.0, 0.0]; n * n],
    };
    window
        .shift_to(
            WindowOrigin {
                block_x: 1,
                block_y: 0,
            },
            strip,
            geometry,
        )
        .unwrap();
    let shifted_populations = window.populations_for_test();
    for q in 0..9 {
        for y in 0..n {
            for x in 0..(n - 32) {
                assert_eq!(
                    shifted_populations[q * n * n + y * n + x],
                    original_populations[q * n * n + y * n + x + 32]
                );
            }
        }
    }
    let overlap = window.sample([20.25, 20.25]).unwrap();
    assert!((overlap.density - before.density).abs() < 1e-6);
    assert!((window.sample([60.25, 20.25]).unwrap().density - 1.0).abs() < 1e-6);
    // The perturbed cell has non-equilibrium populations after streaming.
    let pre = window.populations_for_test();
    let i = 40 * n + 8; // world cell previously at (40, 40)
    let f = std::array::from_fn(|q| pre[q * n * n + i] as f64);
    assert!(f[0].abs() > 1e-4 && f[1].abs() < 1e-7);
    let h = spec.profile.cell_spacing() as f64;
    let dt = config.dt as f64;
    let force = [0.01_f32 as f64 * dt * dt / h, 0.0];
    let omega = 1.0 / (0.5 + 3.0 * config.viscosity as f64 * dt / (h * h));
    let reference = reference_collision(f, force, omega);
    window
        .advance(StepInput {
            acceleration: [0.01, 0.0],
        })
        .unwrap();
    let post = window.populations_for_test();
    let c = [
        [0, 0],
        [1, 0],
        [0, 1],
        [-1, 0],
        [0, -1],
        [1, 1],
        [-1, 1],
        [-1, -1],
        [1, -1],
    ];
    for q in 0..9 {
        let destination = (40_i32 + c[q][1]) as usize * n + (8_i32 + c[q][0]) as usize;
        assert!(
            (post[q * n * n + destination] as f64 - reference[q]).abs() < 2e-7,
            "q={q}, gpu={}, reference={}",
            post[q * n * n + destination],
            reference[q]
        );
    }
    let center = window.sample([30.25, 20.25]).unwrap();
    assert!(center.density.is_finite() && center.density > 0.0);
    assert!((center.velocity[0] - 0.00015).abs() < 2e-5, "{:?}", center);
    let strip = InitialField {
        density: vec![1.0; n * n],
        velocity: vec![[0.0, 0.0]; n * n],
    };
    window
        .shift_to(
            WindowOrigin {
                block_x: 2,
                block_y: 0,
            },
            strip,
            WindowGeometry {
                solid: vec![false; n * n],
                openings: vec![],
            },
        )
        .unwrap();
    let exposed = window.sample([76.25, 20.25]).unwrap();
    assert!(
        exposed.velocity[0].abs() < 1e-6,
        "retained-force strip: {exposed:?}"
    );
    let d = window.snapshot_diagnostics().unwrap();
    assert!(d.density_min > 0.0 && d.max_speed.is_finite());
    eprintln!(
        "functional adapter: {} (software={})",
        d.adapter_name, d.software_adapter
    );
}

// Orthogonal raw-moment f64 reference, used only to check the GPU collision.
fn reference_collision(f: [f64; 9], force: [f64; 2], omega: f64) -> [f64; 9] {
    let c = [
        [0., 0.],
        [1., 0.],
        [0., 1.],
        [-1., 0.],
        [0., -1.],
        [1., 1.],
        [-1., 1.],
        [-1., -1.],
        [1., -1.],
    ];
    let w = [
        4. / 9.,
        1. / 9.,
        1. / 9.,
        1. / 9.,
        1. / 9.,
        1. / 36.,
        1. / 36.,
        1. / 36.,
        1. / 36.,
    ];
    let m = [
        [1., 1., 1., 1., 1., 1., 1., 1., 1.],
        [-4., -1., -1., -1., -1., 2., 2., 2., 2.],
        [4., -2., -2., -2., -2., 1., 1., 1., 1.],
        [0., 1., 0., -1., 0., 1., -1., -1., 1.],
        [0., -2., 0., 2., 0., 1., -1., -1., 1.],
        [0., 0., 1., 0., -1., 1., 1., -1., -1.],
        [0., 0., -2., 0., 2., 1., 1., -1., -1.],
        [0., 1., -1., 1., -1., 0., 0., 0., 0.],
        [0., 0., 0., 0., 0., 1., -1., 1., -1.],
    ];
    let norm = [9., 36., 36., 6., 12., 6., 12., 4., 4.];
    let rates = [0., 1.1, 1., 0., 1.2, 0., 1.2, omega, omega];
    let rho = 1. + f.iter().sum::<f64>();
    let j: [f64; 2] = std::array::from_fn(|k| (0..9).map(|q| c[q][k] * f[q]).sum());
    let u = [(j[0] + force[0] / 2.) / rho, (j[1] + force[1] / 2.) / rho];
    let mut diff = [0.; 9];
    let mut source = [0.; 9];
    for q in 0..9 {
        let cu = c[q][0] * u[0] + c[q][1] * u[1];
        diff[q] = f[q]
            - w[q]
                * ((rho - 1.)
                    + rho * (3. * cu + 4.5 * cu * cu - 1.5 * (u[0] * u[0] + u[1] * u[1])));
        source[q] = w[q]
            * ((3. * (c[q][0] - u[0]) + 9. * cu * c[q][0]) * force[0]
                + (3. * (c[q][1] - u[1]) + 9. * cu * c[q][1]) * force[1]);
    }
    let mut delta = [0.; 9];
    for r in 0..9 {
        let d: f64 = (0..9).map(|q| m[r][q] * diff[q]).sum();
        let z: f64 = (0..9).map(|q| m[r][q] * source[q]).sum();
        delta[r] = -rates[r] * d + (1. - rates[r] / 2.) * z;
    }
    std::array::from_fn(|q| f[q] + (0..9).map(|r| m[r][q] * delta[r] / norm[r]).sum::<f64>())
}
