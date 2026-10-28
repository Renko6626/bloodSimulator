#[path = "lbm/mod.rs"]
mod lbm;
use lbm::*;
fn main() {
    let n = 32;
    let u = 0.022_f32;
    let coarse = std::env::args().any(|a| a == "coarse");
    let profile = if coarse {
        GridProfile::Coarse
    } else {
        GridProfile::Fine
    };
    let h = profile.cell_spacing();
    let dt = (1_f32 / 120.) / if coarse { 4. } else { 7. };
    let world_u = u * h / dt;
    let q = world_u * h * n as f32;
    let geometry = WindowGeometry {
        solid: vec![false; n * n],
        openings: vec![
            OpeningSpec {
                side: WindowSide::Left,
                cells: 0..32,
                direction: [1., 0.],
                outward_flow: -q,
                background_speed: world_u,
            },
            OpeningSpec {
                side: WindowSide::Right,
                cells: 0..32,
                direction: [1., 0.],
                outward_flow: q,
                background_speed: world_u,
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
            velocity: vec![[world_u, 0.]; n * n],
        },
        geometry,
    )
    .unwrap();
    w.set_opening_profiles(&[-q, q], &[vec![-world_u; n], vec![world_u; n]])
        .unwrap();
    let path = std::env::var("FD_MODE_FILE").unwrap_or_else(|_| {
        "docs/research/2026-10-28-lbm-fd-boundary/results/baseline-mode.bin".into()
    });
    let mode: Vec<f32> = std::fs::read(&path)
        .unwrap()
        .chunks_exact(4)
        .map(|c| f32::from_ne_bytes(c.try_into().unwrap()))
        .collect();
    let baseline = w.diagnostic_populations();
    let initial: Vec<f32> = baseline
        .iter()
        .zip(&mode)
        .map(|(a, m)| a + m * 1e-6)
        .collect();
    w.diagnostic_seed(&initial);
    let denom = mode.iter().map(|m| f64::from(*m).powi(2)).sum::<f64>();
    let d = w.snapshot_diagnostics().unwrap();
    println!(
        "mode={} profile={profile:?} dt={dt} tau={} adapter={:?} software={} seed={path}",
        std::env::var("LBMDIAG").unwrap(),
        0.5 + 3. * 0.05 * dt / (h * h),
        d.adapter_name,
        d.software_adapter
    );
    for step in 0..=100 {
        if step % 10 == 0 {
            let pop = w.diagnostic_populations();
            let amp = pop
                .iter()
                .zip(&baseline)
                .zip(&mode)
                .map(|((&a, &b), &m)| f64::from(a - b) * f64::from(m))
                .sum::<f64>()
                / denom;
            let norm = pop
                .iter()
                .zip(&baseline)
                .map(|(&a, &b)| f64::from(a - b).powi(2))
                .sum::<f64>()
                .sqrt();
            let d = w.snapshot_diagnostics().unwrap();
            println!(
                "step={step} amplitude={amp:.10e} total_l2={norm:.10e} rho=[{},{}] mass={} flux={:?}",
                d.density_min, d.density_max, d.mass, d.opening_fluxes
            );
        }
        if step < 100 {
            w.advance(StepInput {
                acceleration: [0.; 2],
            })
            .unwrap();
        }
    }
}
