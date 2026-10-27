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
    density[20 * n] = 1.05;
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
    let opposite = window.sample([47.75, 10.25]).unwrap();
    assert!(
        (opposite.density - 1.0).abs() < 1e-6,
        "no periodic transport: {opposite:?}"
    );
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

#[test]
#[ignore = "explicit software-adapter functional diagnostic"]
fn lbm_opening_flux_and_known_populations() {
    let spec = WindowSpec {
        origin: WindowOrigin {
            block_x: 0,
            block_y: 0,
        },
        profile: GridProfile::Coarse,
    };
    let n = spec.profile.cells_per_side() as usize;
    let mut solid = vec![true; n * n];
    for y in 42..54 {
        for x in 0..n {
            solid[y * n + x] = false;
        }
    }
    for y in 48..n {
        for x in 44..52 {
            solid[y * n + x] = false;
        }
    }
    let openings = vec![
        OpeningSpec {
            side: WindowSide::Left,
            cells: 42..54,
            direction: [1., 0.],
            outward_flow: 0.,
            background_speed: 0.,
        },
        OpeningSpec {
            side: WindowSide::Right,
            cells: 42..54,
            direction: [1., 0.],
            outward_flow: 0.,
            background_speed: 0.,
        },
        OpeningSpec {
            side: WindowSide::Top,
            cells: 44..52,
            direction: [0., 1.],
            outward_flow: 0.,
            background_speed: 0.,
        },
    ];
    let initial = InitialField {
        density: vec![1.; n * n],
        velocity: vec![[0.; 2]; n * n],
    };
    let config = WindowConfig {
        dt: 0.01,
        viscosity: 0.1,
        allow_software: true,
    };
    let mut open = LbmWindow::new(
        spec,
        config,
        initial.clone(),
        WindowGeometry {
            solid: solid.clone(),
            openings,
        },
    )
    .unwrap();
    let mut closed = LbmWindow::new(
        spec,
        config,
        initial,
        WindowGeometry {
            solid,
            openings: vec![],
        },
    )
    .unwrap();
    open.advance(StepInput {
        acceleration: [0., 0.],
    })
    .unwrap();
    let zero = open.snapshot_diagnostics().unwrap();
    assert!(zero.max_speed < 1e-5 && (zero.density_min - 1.).abs() < 1e-6);
    let initial_mass = zero.mass;
    let flows = [-0.6, 0.4, 0.2];
    let profiles = vec![vec![-0.1; 12], vec![0.4 / 6.; 12], vec![0.05; 8]];
    let before = open.populations_for_test();
    open.set_opening_profiles(&flows, &profiles).unwrap();
    assert_eq!(before, open.populations_for_test());
    let force = StepInput {
        acceleration: [0.01, -0.02],
    };
    open.advance(force).unwrap();
    closed
        .advance(StepInput {
            acceleration: [0., 0.],
        })
        .unwrap();
    closed.advance(force).unwrap();
    let actual = open.populations_for_test();
    let reference = closed.populations_for_test();
    let i = 48 * n;
    for q in [0, 2, 3, 4, 6, 7] {
        assert!(
            (actual[q * n * n + i] - reference[q * n * n + i]).abs() < 1e-7,
            "known q={q}"
        );
    }
    let d = open.snapshot_diagnostics().unwrap();
    for (a, b) in d.opening_fluxes.iter().zip(flows) {
        assert!((a - b).abs() < 1e-4, "flux {a} vs {b}");
    }
    assert!(d.density_min > 0. && d.mass.is_finite());
    let p = open.sample([0.25, 24.25]).unwrap();
    assert!(
        (p.velocity[0] - 0.1).abs() < 1e-5 && p.velocity[1].abs() < 1e-5,
        "force-corrected {p:?}"
    );
    for _ in 0..99 {
        open.advance(StepInput {
            acceleration: [0., 0.],
        })
        .unwrap();
    }
    let evolved = open.snapshot_diagnostics().unwrap();
    assert!(evolved.density_min > 0. && evolved.density_max < 2. && evolved.mass.is_finite());
    assert!((evolved.mass - initial_mass).abs() / initial_mass < 0.001);
    for (a, b) in evolved.opening_fluxes.iter().zip(flows) {
        assert!((a - b).abs() < 1e-4);
    }
    eprintln!(
        "fork 100 steps: flux {:?}, initial mass {}, mass {}, density [{}, {}]",
        evolved.opening_fluxes,
        initial_mass,
        evolved.mass,
        evolved.density_min,
        evolved.density_max
    );
    eprintln!(
        "adapter {}, flux {:?}, mass {}, density [{}, {}], force-corrected velocity {:?}",
        d.adapter_name, d.opening_fluxes, d.mass, d.density_min, d.density_max, p.velocity
    );
}

#[test]
#[ignore = "explicit software-adapter functional diagnostic"]
fn lbm_straight_profile_evolves() {
    let spec = WindowSpec {
        origin: WindowOrigin {
            block_x: 0,
            block_y: 0,
        },
        profile: GridProfile::Coarse,
    };
    let n = spec.profile.cells_per_side() as usize;
    let mut solid = vec![true; n * n];
    for y in 42..54 {
        for x in 0..n {
            solid[y * n + x] = false;
        }
    }
    let opening = |side, flow| OpeningSpec {
        side,
        cells: 42..54,
        direction: [1., 0.],
        outward_flow: flow,
        background_speed: 0.,
    };
    let geometry = WindowGeometry {
        solid,
        openings: vec![
            opening(WindowSide::Left, -0.6),
            opening(WindowSide::Right, 0.6),
        ],
    };
    // Analytic planar Poiseuille profile: Q=0.6, half-width R=3.
    let mut velocity = vec![[0.; 2]; n * n];
    for y in 42..54 {
        let lateral = ((y as f32 + 0.5) * 0.5 - 24.) / 3.;
        let speed = 3. * 0.6 / (4. * 3.) * (1. - lateral * lateral);
        for x in 0..n {
            velocity[y * n + x] = [speed, 0.];
        }
    }
    let initial = InitialField {
        density: vec![1.; n * n],
        velocity,
    };
    let mut window = LbmWindow::new(
        spec,
        WindowConfig {
            dt: 0.01,
            viscosity: 0.1,
            allow_software: true,
        },
        initial,
        geometry,
    )
    .unwrap();
    let shape: Vec<f32> = (0..12)
        .map(|j| {
            let x = (j as f32 + 0.5) / 12.;
            x * (1. - x)
        })
        .collect();
    let sum: f32 = shape.iter().sum();
    let velocities: Vec<f32> = shape.iter().map(|v| 0.6 * v / (sum * 0.5)).collect();
    window
        .set_opening_profiles(
            &[-0.6, 0.6],
            &[velocities.iter().map(|v| -v).collect(), velocities.clone()],
        )
        .unwrap();
    let initial_mass = window.snapshot_diagnostics().unwrap().mass;
    for _ in 0..100 {
        window
            .advance(StepInput {
                acceleration: [0., 0.],
            })
            .unwrap();
    }
    let d = window.snapshot_diagnostics().unwrap();
    let mut error = 0_f32;
    for (j, v) in velocities.iter().enumerate() {
        let p = window.sample([0.25, (42 + j) as f32 * 0.5 + 0.25]).unwrap();
        error = error.max((p.velocity[0] - v).abs());
    }
    let mut interior_error = 0_f32;
    for j in 0..12 {
        let lateral = ((j as f32 + 0.5) * 0.5 - 3.) / 3.;
        let expected = 3. * 0.6 / (4. * 3.) * (1. - lateral * lateral);
        let p = window
            .sample([24.25, (42 + j) as f32 * 0.5 + 0.25])
            .unwrap();
        interior_error = interior_error.max((p.velocity[0] - expected).abs());
    }
    let lower = window.sample([24.25, 21.25]).unwrap();
    let upper = window.sample([24.25, 26.75]).unwrap();
    let wall_normal = lower.velocity[1].abs().max(upper.velocity[1].abs());
    eprintln!(
        "interior profile error {interior_error}, wall-normal {wall_normal}, initial mass {initial_mass}, final {}",
        d.mass
    );
    assert!(interior_error < 0.003 && wall_normal < 0.001);
    assert!((d.mass - initial_mass).abs() / initial_mass < 0.001);
    assert!(error < 1e-5 && d.density_min > 0. && d.density_max < 2. && d.mass.is_finite());
    assert!((d.opening_fluxes[0] + 0.6).abs() < 1e-4 && (d.opening_fluxes[1] - 0.6).abs() < 1e-4);
    assert_eq!(
        window.sample([10.25, 10.25]).unwrap().status,
        LocalSampleStatus::SolidOrInvalid
    );
    eprintln!(
        "straight 100 steps: flux {:?}, mass {}, density [{}, {}], max profile error {}",
        d.opening_fluxes, d.mass, d.density_min, d.density_max, error
    );
}
