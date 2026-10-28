#[path = "lbm/mod.rs"]
mod lbm;
use blood_fluid_gpu::lbm as prod;
use lbm::*;
use simulation::{
    Config,
    circulation::graph::GraphNetwork,
    lbm_geometry::{opening_profiles, rasterize},
    map::{MapDefinition, VesselEdge},
};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let fine = !args.iter().any(|s| s == "coarse");
    let fork = args.iter().any(|s| s == "fork");
    let frozen = args.iter().any(|s| s == "frozen");
    let config = Config::default();
    let profile = if fine {
        GridProfile::Fine
    } else {
        GridProfile::Coarse
    };
    let h = profile.cell_spacing();
    let dt = config.dt / config.substeps as f32;
    let count = (dt / (0.05 * h / 10.).min(0.5 * h * h / config.viscosity)).ceil() as usize;
    let ldt = dt / count as f32;
    let mut nodes = vec![[-50., 24.], [100., 24.], [100., 80.], [-50., 80.]];
    let mut pairs = vec![(0, 1), (1, 2), (2, 3), (3, 0)];
    if fork {
        nodes = vec![
            [-50., 24.],
            [32., 24.],
            [100., 24.],
            [100., 80.],
            [-50., 80.],
            [32., 80.],
        ];
        pairs = vec![(0, 1), (1, 2), (2, 3), (3, 5), (5, 4), (4, 0), (1, 5)];
    }
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
        spawn: [31.5, 24.],
        landmarks: vec![],
    };
    let network = GraphNetwork::new(&map, config.viscosity);
    let flow = network.solve(config.pressure);
    let spec = WindowSpec {
        origin: WindowOrigin {
            block_x: 0,
            block_y: 0,
        },
        profile,
    };
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
    let raster = rasterize(&map, &flow, pspec).unwrap();
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
    let mask = raster.geometry.solid.clone();
    let mut w = LbmWindow::new(
        spec,
        WindowConfig {
            dt: ldt,
            viscosity: config.viscosity,
            allow_software: true,
        },
        InitialField {
            density: raster.initial.density.clone(),
            velocity: raster.initial.velocity.clone(),
        },
        WindowGeometry {
            solid: mask.clone(),
            openings: openings.clone(),
        },
    )
    .unwrap();
    let n = profile.cells_per_side() as usize;
    let initial_mass = mask.iter().filter(|v| !**v).count() as f64;
    println!(
        "fine={fine} fork={fork} frozen={frozen} count={count} dt={ldt} tau={} nu={} adapter={:?}",
        0.5 + 3. * config.viscosity * ldt / (h * h),
        config.viscosity,
        w.snapshot_diagnostics().unwrap().adapter_name
    );
    for frame in 0..160 {
        let pressure = config.pressure
            * (1.
                + if frozen {
                    0.
                } else {
                    0.15 * (std::f32::consts::TAU * config.pulse_hz * frame as f32 * config.dt)
                        .sin()
                });
        let flow = network.solve(pressure);
        let (targets, profiles) = opening_profiles(&map, &flow, &raster, pspec).unwrap();
        w.set_opening_profiles(&targets, &profiles).unwrap();
        for k in 0..count * 2 {
            w.advance(StepInput {
                acceleration: [0.; 2],
            })
            .unwrap();
            let f = w.field().unwrap();
            let mut bad = None;
            let mut lo = (f32::INFINITY, 0);
            let mut hi = (f32::NEG_INFINITY, 0);
            let mut maxu = (0_f32, 0);
            for (i, p) in f.iter().enumerate() {
                if mask[i] {
                    continue;
                }
                if p[0] <= 0. || !p[..3].iter().all(|v| v.is_finite()) {
                    bad = Some((i, *p));
                    break;
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
            if let Some((i, p)) = bad {
                println!(
                    "BAD frame={frame} k={k} step={} world=[{},{}] cell=[{},{}] p={p:?}",
                    frame * count * 2 + k + 1,
                    (i % n) as f32 * h + h / 2.,
                    (i / n) as f32 * h + h / 2.,
                    i % n,
                    i / n
                );
                return;
            }
            if k == count * 2 - 1 {
                let d = w.diagnostics_from_field(&f).unwrap();
                let mass = initial_mass
                    + f.iter()
                        .zip(&mask)
                        .filter(|(_, wall)| !**wall)
                        .map(|(p, _)| f64::from(p[3]))
                        .sum::<f64>();
                let mut err = 0_f32;
                for (o, profile) in openings.iter().zip(&profiles) {
                    let normal = match o.side {
                        WindowSide::Left => [-1., 0.],
                        WindowSide::Right => [1., 0.],
                        WindowSide::Bottom => [0., -1.],
                        WindowSide::Top => [0., 1.],
                    };
                    let dot = o.direction[0] * normal[0] + o.direction[1] * normal[1];
                    for (c, &v) in o.cells.clone().zip(profile) {
                        let (x, y) = match o.side {
                            WindowSide::Left => (0, c as usize),
                            WindowSide::Right => (n - 1, c as usize),
                            WindowSide::Bottom => (c as usize, 0),
                            WindowSide::Top => (c as usize, n - 1),
                        };
                        for axis in 0..2 {
                            err = err
                                .max((f[y * n + x][axis + 1] - o.direction[axis] * v / dot).abs());
                        }
                    }
                }
                println!(
                    "measure frame={frame} targets={targets:?} flux={:?} velocity_error={err} mass={mass:.10} mass_rel={:.10e}",
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
                    maxu.0 * ldt / h
                );
            }
        }
    }
    println!("COMPLETE");
}
