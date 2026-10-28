#[path = "../src/lbm/mod.rs"]
mod lbm;
use blood_fluid_gpu::lbm as prod;
use lbm::*;
use simulation::{
    circulation::{Flow, graph::GraphFlow},
    lbm_geometry::{opening_profiles, rasterize},
    map::{MapDefinition, VesselEdge},
};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let name = args.get(1).map(String::as_str).unwrap_or("diagonal45");
    let fine = !args.iter().any(|a| a == "coarse");
    let (nodes, pairs, rates) = match name {
        "diagonal45" => (
            vec![[-50., -38.], [100., 112.]],
            vec![(0, 1)],
            vec![18.529297],
        ),
        "fork_a" => (
            vec![[-50., 24.], [24., 24.], [100., 62.], [-14., 100.]],
            vec![(0, 1), (1, 2), (1, 3)],
            vec![18.529297, 18.529297 * 0.7, 18.529297 * 0.3],
        ),
        "fork_b" => (
            vec![[-50., 5.5], [24., 24.], [100., 5.], [43., 100.]],
            vec![(0, 1), (1, 2), (1, 3)],
            vec![18.529297, 18.529297 * 0.35, 18.529297 * 0.65],
        ),
        _ => panic!("unknown case"),
    };
    let edges = pairs
        .into_iter()
        .map(|(from, to)| VesselEdge {
            from,
            to,
            half_width: 3.,
            centerline: vec![nodes[from], nodes[to]],
        })
        .collect();
    let map = MapDefinition {
        nodes,
        edges,
        pump_edge: 0,
        spawn: [24., 24.],
        landmarks: vec![],
    };
    let flow = GraphFlow {
        pressures: vec![],
        rates,
        summary: Flow {
            pressure_drop: 0.,
            resistance: 0.,
            rate: 18.529297,
            mean_speed: 0.,
        },
    };
    let profile = if fine {
        GridProfile::Fine
    } else {
        GridProfile::Coarse
    };
    let h = profile.cell_spacing();
    let count = if fine { 7 } else { 4 };
    let dt = (1_f32 / 120.) / count as f32;
    let n = profile.cells_per_side() as usize;
    let pspec = prod::WindowSpec {
        origin: prod::WindowOrigin {
            block_x: 0,
            block_y: 0,
        },
        profile: if fine {
            prod::GridProfile::Fine
        } else {
            prod::GridProfile::Coarse
        },
    };
    let raster = match rasterize(&map, &flow, pspec) {
        Ok(r) => r,
        Err(e) => {
            println!("UNSUPPORTED rasterizer {name} {profile:?}: {e}");
            std::process::exit(2)
        }
    };
    let openings: Vec<_> = raster
        .geometry
        .openings
        .iter()
        .map(|o| OpeningSpec {
            side: match o.side {
                prod::WindowSide::Left => WindowSide::Left,
                prod::WindowSide::Right => WindowSide::Right,
                prod::WindowSide::Bottom => WindowSide::Bottom,
                prod::WindowSide::Top => WindowSide::Top,
            },
            cells: o.cells.clone(),
            direction: o.direction,
            outward_flow: o.outward_flow,
            background_speed: o.background_speed,
        })
        .collect();
    let geometry = WindowGeometry {
        solid: raster.geometry.solid.clone(),
        openings,
    };
    println!(
        "case={name} profile={profile:?} n={n} dt={dt} tau={} openings={:?}",
        0.5 + 3. * 0.05 * dt / (h * h),
        geometry.openings
    );
    if let Some(i) = args.iter().position(|a| a == "--export") {
        std::fs::write(&args[i + 1], geometry_blob(n, &geometry, false)).unwrap();
        return;
    }
    let mut w = match LbmWindow::new(
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
            density: raster.initial.density.clone(),
            velocity: raster.initial.velocity.clone(),
        },
        geometry.clone(),
    ) {
        Ok(w) => w,
        Err(e) => {
            println!("UNSUPPORTED solver {e}");
            std::process::exit(2)
        }
    };
    let info = w.snapshot_diagnostics().unwrap();
    println!(
        "adapter={:?} software={}",
        info.adapter_name, info.software_adapter
    );
    if args.iter().any(|a| a == "--manufactured") {
        let c: [[f32; 2]; 9] = [
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
        let weights = [
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
        let blob = geometry_blob(n, &geometry, false);
        let sides = &blob[16 + n * n..];
        for quad in [false, true] {
            let field: Vec<_> = (0..n * n)
                .map(|i| analytic((i % n) as f32 / n as f32, (i / n) as f32 / n as f32, quad).0)
                .collect();
            let pops: Vec<_> = (0..9 * n * n)
                .map(|i| {
                    let q = i / (n * n);
                    let u = field[i % (n * n)];
                    3. * weights[q] * (c[q][0] * u[0] + c[q][1] * u[1])
                })
                .collect();
            w.diagnostic_seed(&pops);
            let target: Vec<_> = sides
                .iter()
                .enumerate()
                .map(|(i, &s)| [s as f32, field[i][0], field[i][1], 0.])
                .collect();
            w.diagnostic_boundary(&target);
            let observed = w.diagnostic_gradient().unwrap();
            let mut err = 0_f32;
            let mut observed_count = 0;
            for (i, &side) in sides.iter().enumerate() {
                if side == 0 {
                    continue;
                }
                let (_, dx, dy) =
                    analytic((i % n) as f32 / n as f32, (i / n) as f32 / n as f32, quad);
                let normal = match side {
                    1 => [1., 0.],
                    2 => [-1., 0.],
                    3 => [0., 1.],
                    _ => [0., -1.],
                };
                let tangent = [-normal[1], normal[0]];
                let expected = [
                    (normal[0] * dx[0] + normal[1] * dy[0]) / n as f32,
                    (normal[0] * dx[1] + normal[1] * dy[1]) / n as f32,
                    (tangent[0] * dx[0] + tangent[1] * dy[0]) / n as f32,
                    (tangent[0] * dx[1] + tangent[1] * dy[1]) / n as f32,
                ];
                for axis in 0..4 {
                    err = err.max((observed[i][axis] - expected[axis]).abs());
                }
                observed_count += 1;
            }
            println!(
                "MANUFACTURED quadratic={quad} opening_cells={observed_count} max_absolute_gradient_error={err:.10e} tolerance=5e-8"
            );
            assert!(err <= 5e-8);
        }
        return;
    }
    let initial_mass = geometry.solid.iter().filter(|s| !**s).count() as f64;
    for frame in 0..1000 {
        let mut next = flow.clone();
        let pulse = 1. + 0.15 * (std::f32::consts::TAU * 1.2 * frame as f32 / 60.).sin();
        for q in &mut next.rates {
            *q *= pulse;
        }
        let (targets, profiles) = opening_profiles(&map, &next, &raster, pspec).unwrap();
        w.set_opening_profiles(&targets, &profiles).unwrap();
        for substep in 0..count * 2 {
            w.advance(StepInput {
                acceleration: [0.; 2],
            })
            .unwrap();
            let field = w.field().unwrap();
            for (i, p) in field.iter().enumerate() {
                if geometry.solid[i] {
                    continue;
                }
                if p[0] <= 0. || !p[..3].iter().all(|x| x.is_finite()) {
                    println!(
                        "BAD frame={frame} substep={substep} step={} cell=[{},{}] field={p:?}",
                        frame * count * 2 + substep + 1,
                        i % n,
                        i / n
                    );
                    std::process::exit(2);
                }
            }
            if substep + 1 == count * 2 {
                let d = w.diagnostics_from_field(&field).unwrap();
                let mass = initial_mass
                    + field
                        .iter()
                        .zip(&geometry.solid)
                        .filter(|(_, solid)| !**solid)
                        .map(|(p, _)| f64::from(p[3]))
                        .sum::<f64>();
                let mut verr = 0_f32;
                for (o, pf) in geometry.openings.iter().zip(&profiles) {
                    let normal = match o.side {
                        WindowSide::Left => [-1., 0.],
                        WindowSide::Right => [1., 0.],
                        WindowSide::Bottom => [0., -1.],
                        WindowSide::Top => [0., 1.],
                    };
                    let dot = o.direction[0] * normal[0] + o.direction[1] * normal[1];
                    for (c, &v) in o.cells.clone().zip(pf) {
                        let (x, y) = match o.side {
                            WindowSide::Left => (0, c as usize),
                            WindowSide::Right => (n - 1, c as usize),
                            WindowSide::Bottom => (c as usize, 0),
                            WindowSide::Top => (c as usize, n - 1),
                        };
                        for axis in 0..2 {
                            verr = verr.max(
                                (field[y * n + x][axis + 1] - o.direction[axis] * v / dot).abs(),
                            );
                        }
                    }
                }
                let ferr = d
                    .opening_fluxes
                    .iter()
                    .zip(&targets)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0_f32, f32::max);
                println!(
                    "frame={frame} rho=[{},{}] mass_rel={:.10e} max_lattice={} velocity_error={verr} flux_error={ferr} targets={targets:?} flux={:?}",
                    d.density_min,
                    d.density_max,
                    (mass - initial_mass) / initial_mass,
                    d.max_speed * dt / h,
                    d.opening_fluxes
                );
            }
        }
    }
    println!("COMPLETE");
}
fn analytic(x: f32, y: f32, quadratic: bool) -> ([f32; 2], [f32; 2], [f32; 2]) {
    let q = if quadratic { 1. } else { 0. };
    (
        [
            0.015 + 0.004 * x - 0.003 * y + q * (0.002 * x * x + 0.003 * x * y - 0.001 * y * y),
            -0.002 + 0.002 * x + 0.005 * y + q * (-0.0015 * x * x + 0.002 * x * y + 0.001 * y * y),
        ],
        [
            0.004 + q * (0.004 * x + 0.003 * y),
            0.002 + q * (-0.003 * x + 0.002 * y),
        ],
        [
            -0.003 + q * (0.003 * x - 0.002 * y),
            0.005 + q * (0.002 * x + 0.002 * y),
        ],
    )
}
