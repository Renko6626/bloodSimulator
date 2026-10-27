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
    window
        .advance(StepInput {
            acceleration: [0.01, 0.0],
        })
        .unwrap();
    let center = window.sample([30.25, 20.25]).unwrap();
    assert!(center.density.is_finite() && center.density > 0.0);
    assert!((center.velocity[0] - 0.00015).abs() < 2e-5, "{:?}", center);
    let d = window.snapshot_diagnostics().unwrap();
    assert!(d.density_min > 0.0 && d.max_speed.is_finite());
    eprintln!(
        "functional adapter: {} (software={})",
        d.adapter_name, d.software_adapter
    );
}
