use crate::model::*;
#[test]
fn periodic_force_transfers_expected_momentum_without_mass_change() {
    let c = Config::new(16, 16, 4., 4., 0.1, true, 2., 1);
    let mut f = Cpu::new(c, |_, _| [0., 0.], |_, _| 1., [0.2, -0.07]);
    for _ in 0..40 {
        f.step([0.2, -0.07]);
    }
    let field = f.field([0.2, -0.07]);
    for p in field.samples {
        assert!((p[0] - 1.).abs() < 1e-12);
        assert!((p[1] - 0.2 * 40. * c.dt).abs() < 1e-11);
        assert!((p[2] + 0.07 * 40. * c.dt).abs() < 1e-11);
    }
}
#[test]
fn wall_channel_preserves_profile_with_small_discretization_error() {
    let c = Config::new(32, 16, 8., 4., 0.3, false, 2., 1);
    let a = 12. * c.nu * 0.7 / (c.ly * c.ly);
    let exact = |_: f64, y: f64| [a * y * (c.ly - y) / (2. * c.nu), 0.];
    let mut f = Cpu::new(c, exact, |_, _| 1., [a, 0.]);
    for _ in 0..800 {
        f.step([a, 0.]);
    }
    assert!(f.field([a, 0.]).relative_error(c, exact, [0., 0.]) < 0.015);
}
#[test]
fn impulse_kick_preserves_density_and_adds_prescribed_momentum() {
    let c = Config::new(8, 8, 2., 2., 0.1, true, 2., 1);
    let mut f = Cpu::new(c, |_, _| [0., 0.], |_, _| 1., [0., 0.]);
    let mut kicks = vec![[0., 0.]; c.len()];
    kicks[11] = [0.03, -0.01];
    f.kick(&kicks);
    let before = f.field([0., 0.]);
    assert!((before.samples[11][0] - 1.).abs() < 1e-13);
    assert!((before.samples[11][1] * c.dt / c.h - 0.03).abs() < 1e-13);
    for _ in 0..20 {
        f.step([0., 0.]);
    }
    let after = f.field([0., 0.]);
    let p = after.samples.iter().fold([0., 0.], |a, p| {
        [
            a[0] + p[0] * p[1] * c.dt / c.h,
            a[1] + p[0] * p[2] * c.dt / c.h,
        ]
    });
    assert!((p[0] - 0.03).abs() < 1e-12 && (p[1] + 0.01).abs() < 1e-12);
}
#[test]
fn gpu_shader_validates() {
    let m = wgpu::naga::front::wgsl::parse_str(include_str!("lbm.wgsl")).unwrap();
    wgpu::naga::valid::Validator::new(
        wgpu::naga::valid::ValidationFlags::all(),
        wgpu::naga::valid::Capabilities::empty(),
    )
    .validate(&m)
    .unwrap();
}
#[test]
fn cell_and_fluid_receive_opposite_actual_impulses() {
    let c = Config::new(32, 32, 8., 8., 0.1, false, 2., 1);
    let mut fluid = Cpu::new(c, |_, _| [0.2, -0.1], |_, _| 1., [0., 0.]);
    let mut cells = crate::coupling::Cells::new(c, 1);
    let before = fluid.field([0., 0.]);
    let (k, m) = cells.exchange(c, &before);
    assert!(m < 1e-9);
    let k: Vec<[f64; 2]> = k.iter().map(|x| [x[0] as f64, x[1] as f64]).collect();
    fluid.kick(&k);
    let after = fluid.field([0., 0.]);
    let mut total = cells.momentum();
    for (a, b) in after.samples.iter().zip(&before.samples) {
        for d in 0..2 {
            total[d] += (a[0] * a[d + 1] - b[0] * b[d + 1]) * c.h * c.h;
        }
    }
    assert!(total[0].hypot(total[1]) < 1e-9);
}
