// Existing rasterizer capability; no geometry code is modified.
#[path = "lbm/mod.rs"]
mod lbm;
use blood_fluid_gpu::lbm as prod;
use simulation::{
    circulation::{Flow, graph::GraphFlow},
    lbm_geometry::rasterize,
    map::{MapDefinition, VesselEdge},
};
fn main() {
    for fine in [true, false] {
        let nodes = vec![[-50., -38.], [100., 112.]];
        let map = MapDefinition {
            edges: vec![VesselEdge {
                from: 0,
                to: 1,
                half_width: 3.,
                centerline: nodes.clone(),
            }],
            nodes,
            pump_edge: 0,
            spawn: [24., 36.],
            landmarks: vec![],
        };
        let flow = GraphFlow {
            pressures: vec![],
            rates: vec![18.529297],
            summary: Flow {
                pressure_drop: 0.,
                resistance: 0.,
                rate: 18.529297,
                mean_speed: 0.,
            },
        };
        let profile = if fine {
            prod::GridProfile::Fine
        } else {
            prod::GridProfile::Coarse
        };
        let pspec = prod::WindowSpec {
            origin: prod::WindowOrigin {
                block_x: 0,
                block_y: 0,
            },
            profile,
        };
        let r = rasterize(&map, &flow, pspec).unwrap();
        println!(
            "profile={profile:?} diagonal45 openings={:?}",
            r.geometry.openings
        );
        let openings = r
            .geometry
            .openings
            .iter()
            .map(|o| lbm::OpeningSpec {
                side: match o.side {
                    prod::WindowSide::Left => lbm::WindowSide::Left,
                    prod::WindowSide::Right => lbm::WindowSide::Right,
                    prod::WindowSide::Top => lbm::WindowSide::Top,
                    prod::WindowSide::Bottom => lbm::WindowSide::Bottom,
                },
                cells: o.cells.clone(),
                direction: o.direction,
                outward_flow: o.outward_flow,
                background_speed: o.background_speed,
            })
            .collect();
        let result = lbm::LbmWindow::new(
            lbm::WindowSpec {
                origin: lbm::WindowOrigin {
                    block_x: 0,
                    block_y: 0,
                },
                profile: if fine {
                    lbm::GridProfile::Fine
                } else {
                    lbm::GridProfile::Coarse
                },
            },
            lbm::WindowConfig {
                dt: (1_f32 / 120.) / if fine { 7. } else { 4. },
                viscosity: 0.05,
                allow_software: true,
            },
            lbm::InitialField {
                density: r.initial.density,
                velocity: r.initial.velocity,
            },
            lbm::WindowGeometry {
                solid: r.geometry.solid,
                openings,
            },
        );
        match result {
            Ok(_) => println!("STENCIL_SUPPORTED"),
            Err(e) => println!("UNSUPPORTED {e}"),
        };
    }
}
