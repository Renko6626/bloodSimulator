#[path = "lbm/mod.rs"]
mod lbm;
use lbm::*;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let fine = !args.iter().any(|s| s == "coarse");
    let frozen = args.iter().any(|s| s == "frozen");
    let forced = args.iter().any(|s| s == "forced");
    let frames = if forced { 4 } else { 160 };
    let profile = if fine {
        GridProfile::Fine
    } else {
        GridProfile::Coarse
    };
    let h = profile.cell_spacing();
    let n = profile.cells_per_side() as usize;
    let count = if fine { 7 } else { 4 };
    let dt = (1_f32 / 120.) / count as f32;
    let q0 = 18.529297_f32;
    let start = (21. / h) as u32;
    let end = (27. / h) as u32;
    let mut solid = vec![];
    let mut velocity = vec![];
    for _y in 0..n {
        let y = (_y as f32 + 0.5) * h;
        for _x in 0..n {
            let wall = (y - 24.).abs() >= 3.;
            solid.push(wall);
            velocity.push(if wall {
                [0.; 2]
            } else {
                [q0 / 4. * (1. - ((y - 24.) / 3.).powi(2)), 0.]
            });
        }
    }
    let geom = WindowGeometry {
        solid: solid.clone(),
        openings: vec![
            OpeningSpec {
                side: WindowSide::Left,
                cells: start..end,
                direction: [1., 0.],
                outward_flow: -q0,
                background_speed: q0 / 4.,
            },
            OpeningSpec {
                side: WindowSide::Right,
                cells: start..end,
                direction: [1., 0.],
                outward_flow: q0,
                background_speed: q0 / 4.,
            },
        ],
    };
    let mut w = LbmWindow::new(
        WindowSpec {
            origin: WindowOrigin {
                block_x: 0,
                block_y: 0,
            },
            profile,
        },
        WindowConfig {
            dt,
            viscosity: 0.05,
            allow_software: true,
        },
        InitialField {
            density: vec![1.; n * n],
            velocity,
        },
        geom,
    )
    .unwrap();
    let initial_mass = solid.iter().filter(|v| !**v).count() as f64;
    let info = w.snapshot_diagnostics().unwrap();
    println!(
        "adapter={:?} software={} initial_mass={initial_mass}",
        info.adapter_name, info.software_adapter
    );
    println!(
        "mode={} fine={fine} frozen={frozen} dt={dt} tau={}",
        std::env::var("LBMDIAG").unwrap_or_default(),
        0.5 + 3. * 0.05 * dt / (h * h)
    );
    for frame in 0..frames {
        let q = q0
            * (1.
                + if frozen {
                    0.
                } else {
                    0.15 * (std::f32::consts::TAU * 1.2 * frame as f32 / 60.).sin()
                });
        let weights: Vec<f32> = (start..end)
            .map(|j| {
                let y = (j as f32 + 0.5) * h;
                (1. - ((y - 24.) / 3.).powi(2)).max(0.)
            })
            .collect();
        let sum = weights.iter().sum::<f32>() * h;
        let profiles = vec![
            weights.iter().map(|v| -q * v / sum).collect(),
            weights.iter().map(|v| q * v / sum).collect(),
        ];
        w.set_opening_profiles(&[-q, q], &profiles).unwrap();
        for k in 0..count * 2 {
            w.advance(StepInput {
                acceleration: if forced { [10., -7.] } else { [0.; 2] },
            })
            .unwrap();
            let f = w.field().unwrap();
            let mut lo = (f32::INFINITY, 0);
            let mut hi = (f32::NEG_INFINITY, 0);
            let mut maxu = (0_f32, 0);
            for (i, p) in f.iter().enumerate() {
                if solid[i] {
                    continue;
                }
                if p[0] <= 0. || !p[..3].iter().all(|v| v.is_finite()) {
                    println!(
                        "BAD frame={frame} k={k} step={} cell=[{},{}] p={p:?}",
                        frame * count * 2 + k + 1,
                        i % n,
                        i / n
                    );
                    return;
                }
                if p[0] < lo.0 {
                    lo = (p[0], i)
                }
                if p[0] > hi.0 {
                    hi = (p[0], i)
                }
                if p[1].hypot(p[2]) > maxu.0 {
                    maxu = (p[1].hypot(p[2]), i)
                }
            }
            if k == count * 2 - 1 {
                let d = w.diagnostics_from_field(&f).unwrap();
                let mass = f
                    .iter()
                    .zip(&solid)
                    .filter(|(_, wall)| !**wall)
                    .map(|(p, _)| f64::from(p[3]))
                    .sum::<f64>()
                    + initial_mass;
                let mut verr = 0_f32;
                for (j, &v) in profiles[1].iter().enumerate() {
                    let y = start as usize + j;
                    for x in [0, n - 1] {
                        let p = f[y * n + x];
                        verr = verr.max((p[1] - v).abs()).max(p[2].abs());
                    }
                }
                println!(
                    "measure frame={frame} target={q} flux={:?} velocity_error={verr} mass={mass:.10} mass_rel={:.10e}",
                    d.opening_fluxes,
                    (mass - initial_mass) / initial_mass
                );
                println!(
                    "frame={frame} lo={} at=[{},{}] hi={} at=[{},{}] maxu={} at=[{},{}] ulat={}",
                    lo.0,
                    lo.1 % n,
                    lo.1 / n,
                    hi.0,
                    hi.1 % n,
                    hi.1 / n,
                    maxu.0,
                    maxu.1 % n,
                    maxu.1 / n,
                    maxu.0 * dt / h
                );
            }
        }
    }
    println!("COMPLETE");
}
