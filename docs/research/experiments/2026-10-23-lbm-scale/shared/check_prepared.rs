#[path = "../../2026-10-23-lbm-dynamics/shared/fluid.rs"]
mod baseline;
#[path = "fluid.rs"]
mod cached;

fn compare(a: &mut baseline::Fluid, b: &mut cached::Fluid, data: &[[f64; 5]], step: usize) {
    let old: Vec<_> = data
        .iter()
        .map(|m| baseline::Marker {
            position: [m[0], m[1]],
            velocity: [m[2], m[3]],
            drag: m[4],
        })
        .collect();
    let new: Vec<_> = data
        .iter()
        .map(|m| cached::Marker {
            position: [m[0], m[1]],
            velocity: [m[2], m[3]],
            drag: m[4],
        })
        .collect();
    let prepared = b.prepare_markers(&new).unwrap();
    for tick in 0..step {
        let x = a.advance(&old).unwrap();
        let y = b.advance_prepared(&prepared).unwrap();
        assert_eq!(x.marker_impulses.len(), y.marker_impulses.len());
        for (j, (old, new)) in x.marker_impulses.iter().zip(&y.marker_impulses).enumerate() {
            for d in 0..2 {
                assert_eq!(old[d].to_bits(), new[d].to_bits(), "marker {j} tick {tick}");
            }
        }
        for d in 0..2 {
            assert_eq!(
                x.fluid_impulse[d].to_bits(),
                y.fluid_impulse[d].to_bits(),
                "fluid impulse tick {tick}"
            );
            assert_eq!(
                x.impulse_error[d].to_bits(),
                y.impulse_error[d].to_bits(),
                "impulse error tick {tick}"
            );
        }
        assert_eq!(
            x.max_kernel_weight_error.to_bits(),
            y.max_kernel_weight_error.to_bits()
        );
        let af = a.field();
        let bf = b.field();
        for (i, (u, v)) in af.iter().zip(&bf).enumerate() {
            assert_eq!(u.rho.to_bits(), v.rho.to_bits(), "rho cell {i} tick {tick}");
            for d in 0..2 {
                assert_eq!(
                    u.velocity[d].to_bits(),
                    v.velocity[d].to_bits(),
                    "velocity cell {i} axis {d} tick {tick}"
                );
            }
        }
    }
}

fn main() {
    let mut a = baseline::Fluid::new(32, 2.0, 0.25);
    let mut b = cached::Fluid::new(32, 2.0, 0.25);
    assert_eq!(a.dt().to_bits(), b.dt().to_bits());
    compare(
        &mut a,
        &mut b,
        &[
            [0.18, 0.35, 0.05, 0.0, 0.5],
            [0.41, 0.65, -0.03, 0.02, 0.5],
            [0.61, 0.35, 0.01, -0.02, 0.5],
            [0.82, 0.65, 0.0, 0.01, 0.5],
        ],
        17,
    );
    compare(
        &mut a,
        &mut b,
        &[
            [0.992, 0.35, 0.05, 0.0, 0.5],
            [0.008, 0.65, -0.03, 0.02, 0.5],
            [0.63, 0.38, 0.01, -0.02, 0.5],
            [0.80, 0.62, 0.0, 0.01, 0.5],
        ],
        18,
    );
    println!("PASS: 17+18 steps; bitwise identical field and impulses after marker movement and seam crossing");
}
