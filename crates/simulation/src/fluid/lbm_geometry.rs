//! Map-derived geometry and planar background for LBM windows.
use crate::{circulation::graph::GraphFlow, map::MapDefinition};
use blood_fluid_gpu::lbm::{InitialField, OpeningSpec, WindowGeometry, WindowSide, WindowSpec};
use rapier2d::prelude::Vector;
use std::collections::VecDeque;

pub use crate::background::EQUIVALENT_DEPTH;

#[derive(Clone, Debug)]
pub struct OpeningGeometry {
    pub segment: usize,
    pub endpoints: [[f32; 2]; 2],
    pub normal: [f32; 2],
    /// Outward normal velocity at each opening cell, normalized to the target flux.
    pub normal_velocity: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct MapWindowGeometry {
    pub geometry: WindowGeometry,
    pub initial: InitialField,
    pub openings: Vec<OpeningGeometry>,
}

/// Rasterize shared map geometry. Openings require a single straight segment
/// crossing the boundary away from its endpoints, and may not touch a corner.
pub fn rasterize(
    map: &MapDefinition,
    flow: &GraphFlow,
    spec: WindowSpec,
) -> Result<MapWindowGeometry, String> {
    if map.edges.is_empty()
        || flow.rates.len() != map.edges.len()
        || flow.rates.iter().any(|v| !v.is_finite())
    {
        return Err("invalid map or graph flow".into());
    }
    let n = spec.profile.cells_per_side() as usize;
    let h = spec.profile.cell_spacing();
    let origin = spec.world_bounds()[0];
    let point = |x: usize, y: usize| {
        Vector::new(
            origin[0] + (x as f32 + 0.5) * h,
            origin[1] + (y as f32 + 0.5) * h,
        )
    };
    let mut solid = Vec::with_capacity(n * n);
    let mut velocity = Vec::with_capacity(n * n);
    for y in 0..n {
        for x in 0..n {
            let p = point(x, y);
            let wall = map.signed_distance(p) >= 0.0;
            solid.push(wall);
            if wall {
                velocity.push([0.0, 0.0]);
                continue;
            }
            let edge = map.nearest_edge(p);
            let proj = map.project_edge(edge, p);
            let radius = map.edges[edge].half_width;
            if !radius.is_finite() || radius <= 0.0 {
                return Err("invalid vessel radius".into());
            }
            let shape = (1.0 - (proj.distance / radius).powi(2)).max(0.0);
            velocity.push(
                (proj.tangent
                    * (3.0 * flow.rates[edge] * shape / (4.0 * radius * EQUIVALENT_DEPTH)))
                    .to_array(),
            );
        }
    }
    let mut specs = Vec::new();
    let mut metadata = Vec::new();
    for (side, normal) in [
        (WindowSide::Left, Vector::NEG_X),
        (WindowSide::Right, Vector::X),
        (WindowSide::Bottom, Vector::NEG_Y),
        (WindowSide::Top, Vector::Y),
    ] {
        let xy = |i: usize| match side {
            WindowSide::Left => (0, i),
            WindowSide::Right => (n - 1, i),
            WindowSide::Bottom => (i, 0),
            WindowSide::Top => (i, n - 1),
        };
        let boundary_point = |i: f32| match side {
            WindowSide::Left => Vector::new(origin[0], origin[1] + i * h),
            WindowSide::Right => Vector::new(origin[0] + n as f32 * h, origin[1] + i * h),
            WindowSide::Bottom => Vector::new(origin[0] + i * h, origin[1]),
            WindowSide::Top => Vector::new(origin[0] + i * h, origin[1] + n as f32 * h),
        };
        let mut i = 0;
        while i < n {
            let (x, y) = xy(i);
            if solid[y * n + x] {
                i += 1;
                continue;
            }
            let start = i;
            while i < n {
                let (x, y) = xy(i);
                if solid[y * n + x] {
                    break;
                }
                i += 1;
            }
            let end = i;
            if start == 0 || end == n {
                return Err("unsupported opening at window corner".into());
            }
            let center = boundary_point((start + end) as f32 / 2.0);
            let edge = map.nearest_edge(center);
            let e = &map.edges[edge];
            let proj = map.project_edge(edge, center);
            let tangent = proj.tangent;
            let normal_dot = tangent.dot(normal);
            if normal_dot.abs() < 0.1 || proj.along < h || map.edge_length(edge) - proj.along < h {
                return Err("unsupported opening without straight buffer".into());
            }
            let mut buffer_start = proj.along - h;
            let mut buffer_end = proj.along + h;
            let mut weights = Vec::new();
            for j in start..end {
                let p = boundary_point(j as f32 + 0.5);
                if map.nearest_edge(p) != edge {
                    return Err("ambiguous opening segments".into());
                }
                let projection = map.project_edge(edge, p);
                buffer_start = buffer_start.min(projection.along - h);
                buffer_end = buffer_end.max(projection.along + h);
                let d = projection.distance;
                weights.push((1.0 - (d / e.half_width).powi(2)).max(0.0));
            }
            if buffer_start < 0.0 || buffer_end > map.edge_length(edge) {
                return Err("unsupported opening without straight buffer".into());
            }
            let mut along = 0.0;
            for pair in e.centerline.windows(2) {
                let delta = Vector::from(pair[1]) - Vector::from(pair[0]);
                let length = delta.length();
                if length < 1e-6 {
                    return Err("unsupported curved opening".into());
                }
                if along < buffer_end
                    && along + length > buffer_start
                    && delta.normalize().dot(tangent) < 0.9999
                {
                    return Err("unsupported curved opening".into());
                }
                along += length;
            }
            let sum: f32 = weights.iter().sum();
            if sum <= 0.0 {
                return Err("opening has no profile support".into());
            }
            let flux = flow.rates[edge] / EQUIVALENT_DEPTH * normal_dot.signum();
            let normal_velocity = weights.into_iter().map(|w| flux * w / (sum * h)).collect();
            specs.push(OpeningSpec {
                side,
                cells: start as u32..end as u32,
                direction: tangent.to_array(),
                outward_flow: flux,
                background_speed: 3.0 * flow.rates[edge] / (4.0 * e.half_width * EQUIVALENT_DEPTH),
            });
            metadata.push(OpeningGeometry {
                segment: edge,
                endpoints: [
                    boundary_point(start as f32).to_array(),
                    boundary_point(end as f32).to_array(),
                ],
                normal: normal.to_array(),
                normal_velocity,
            });
        }
    }
    let mut visited = vec![false; n * n];
    for k in 0..n * n {
        if solid[k] || visited[k] {
            continue;
        }
        let mut queue = VecDeque::from([k]);
        visited[k] = true;
        let mut touches = false;
        while let Some(c) = queue.pop_front() {
            let (x, y) = (c % n, c / n);
            if x == 0 || y == 0 || x == n - 1 || y == n - 1 {
                touches = true;
            }
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (a, b) = (x as isize + dx, y as isize + dy);
                if a >= 0 && b >= 0 && a < n as isize && b < n as isize {
                    let j = b as usize * n + a as usize;
                    if !solid[j] && !visited[j] {
                        visited[j] = true;
                        queue.push_back(j);
                    }
                }
            }
        }
        if !touches {
            return Err("fluid component has no usable opening".into());
        }
    }
    Ok(MapWindowGeometry {
        geometry: WindowGeometry {
            solid,
            openings: specs,
        },
        initial: InitialField {
            density: vec![1.0; n * n],
            velocity,
        },
        openings: metadata,
    })
}

/// Recompute opening profiles from cached geometry and the current network flow.
pub fn opening_profiles(
    map: &MapDefinition,
    flow: &GraphFlow,
    raster: &MapWindowGeometry,
    spec: WindowSpec,
) -> Result<(Vec<f32>, Vec<Vec<f32>>), String> {
    if flow.rates.len() != map.edges.len() || flow.rates.iter().any(|v| !v.is_finite()) {
        return Err("invalid graph flow".into());
    }
    let h = spec.profile.cell_spacing();
    let mut targets = Vec::new();
    let mut profiles = Vec::new();
    for (opening, meta) in raster.geometry.openings.iter().zip(&raster.openings) {
        let edge = &map.edges[meta.segment];
        let normal_dot =
            opening.direction[0] * meta.normal[0] + opening.direction[1] * meta.normal[1];
        let flux = flow.rates[meta.segment] / EQUIVALENT_DEPTH * normal_dot.signum();
        let count = (opening.cells.end - opening.cells.start) as usize;
        let weights: Vec<f32> = (0..count)
            .map(|i| {
                let t = (i as f32 + 0.5) / count as f32;
                let p = Vector::new(
                    meta.endpoints[0][0] + t * (meta.endpoints[1][0] - meta.endpoints[0][0]),
                    meta.endpoints[0][1] + t * (meta.endpoints[1][1] - meta.endpoints[0][1]),
                );
                let d = map.project_edge(meta.segment, p).distance;
                (1.0 - (d / edge.half_width).powi(2)).max(0.0)
            })
            .collect();
        let sum: f32 = weights.iter().sum();
        if sum <= 0.0 {
            return Err("opening has no profile support".into());
        }
        targets.push(flux);
        profiles.push(weights.into_iter().map(|w| flux * w / (sum * h)).collect());
    }
    Ok((targets, profiles))
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::{circulation::Flow, map::VesselEdge};
    use blood_fluid_gpu::lbm::{GridProfile, WindowOrigin};
    pub(crate) fn fixture(fork: bool, rates: Vec<f32>) -> (MapDefinition, GraphFlow) {
        let nodes = if fork {
            vec![[-10., 24.], [24., 24.], [58., 24.], [24., 58.]]
        } else {
            vec![[-10., 24.], [58., 24.]]
        };
        let edges = if fork {
            vec![(0, 1, 3.), (1, 2, 2.), (1, 3, 1.)]
        } else {
            vec![(0, 1, 3.)]
        }
        .into_iter()
        .map(|(from, to, half_width)| VesselEdge {
            from,
            to,
            half_width,
            centerline: vec![nodes[from], nodes[to]],
        })
        .collect();
        (
            MapDefinition {
                fluid: None,
                nodes,
                edges,
                pump_edge: 0,
                spawn: [0., 0.],
                landmarks: vec![],
            },
            GraphFlow {
                pressures: vec![],
                rates,
                summary: Flow {
                    pressure_drop: 0.,
                    resistance: 0.,
                    rate: 0.,
                    mean_speed: 0.,
                },
            },
        )
    }
    #[test]
    fn straight_and_fork_flux_on_both_grids() {
        for profile in [GridProfile::Fine, GridProfile::Coarse] {
            let spec = WindowSpec {
                origin: WindowOrigin {
                    block_x: 0,
                    block_y: 0,
                },
                profile,
            };
            for (fork, rates, expected) in [
                (false, vec![6.], vec![-6., 6.]),
                (true, vec![6., 4., 2.], vec![-6., 4., 2.]),
            ] {
                let (map, flow) = fixture(fork, rates);
                let result = rasterize(&map, &flow, spec).unwrap();
                assert_eq!(result.openings.len(), expected.len());
                let mut total = 0.;
                for ((opening, meta), q) in result
                    .geometry
                    .openings
                    .iter()
                    .zip(&result.openings)
                    .zip(expected)
                {
                    assert!((opening.outward_flow - q).abs() < 1e-5);
                    let actual: f32 =
                        meta.normal_velocity.iter().sum::<f32>() * profile.cell_spacing();
                    assert!((actual - q).abs() < 1e-5);
                    total += actual;
                }
                assert!(total.abs() < 1e-5);
                let n = profile.cells_per_side() as usize;
                assert!(result.geometry.solid[0]);
                assert_eq!(result.initial.velocity[0], [0., 0.]);
                assert!(
                    !result.geometry.solid[(24. / profile.cell_spacing()) as usize * n
                        + (5. / profile.cell_spacing()) as usize]
                );
            }
            let (map, flow) = fixture(false, vec![0.]);
            assert!(
                rasterize(&map, &flow, spec)
                    .unwrap()
                    .initial
                    .velocity
                    .iter()
                    .all(|v| *v == [0., 0.])
            );
        }
    }
    #[test]
    #[ignore = "explicit software-adapter functional diagnostic"]
    fn map_fork_profiles_drive_gpu() {
        use blood_fluid_gpu::lbm::{LbmWindow, StepInput, WindowConfig};
        let spec = WindowSpec {
            origin: WindowOrigin {
                block_x: 0,
                block_y: 0,
            },
            profile: GridProfile::Coarse,
        };
        let (map, flow) = fixture(true, vec![0.6, 0.4, 0.2]);
        let raster = rasterize(&map, &flow, spec).unwrap();
        let targets: Vec<_> = raster
            .geometry
            .openings
            .iter()
            .map(|o| o.outward_flow)
            .collect();
        let profiles: Vec<_> = raster
            .openings
            .iter()
            .map(|o| o.normal_velocity.clone())
            .collect();
        let mut window = LbmWindow::new(
            spec,
            WindowConfig {
                dt: 0.01,
                viscosity: 0.1,
                allow_software: true,
            },
            raster.initial,
            raster.geometry,
        )
        .unwrap();
        window.set_opening_profiles(&targets, &profiles).unwrap();
        for _ in 0..100 {
            window
                .advance(StepInput {
                    acceleration: [0., 0.],
                })
                .unwrap();
        }
        let d = window.snapshot_diagnostics().unwrap();
        for (actual, expected) in d.opening_fluxes.iter().zip(&targets) {
            assert!((actual - expected).abs() < 1e-4);
        }
        assert!(d.density_min > 0. && d.density_max < 2. && d.mass.is_finite());
        eprintln!(
            "map fork flux {:?}, mass {}, density [{}, {}]",
            d.opening_fluxes, d.mass, d.density_min, d.density_max
        );
    }
    #[test]
    fn accepts_interior_bends_with_straight_opening_buffers() {
        let spec = WindowSpec {
            origin: WindowOrigin {
                block_x: 0,
                block_y: 0,
            },
            profile: GridProfile::Coarse,
        };
        let (mut map, flow) = fixture(false, vec![1.]);
        map.edges[0].centerline = vec![[-10., 24.], [12., 24.], [24., 30.], [36., 24.], [58., 24.]];
        let result = rasterize(&map, &flow, spec).unwrap();
        assert_eq!(result.geometry.openings.len(), 2);
        assert!((result.geometry.openings[0].outward_flow + 1.).abs() < 1e-5);
        assert!((result.geometry.openings[1].outward_flow - 1.).abs() < 1e-5);
    }
    #[test]
    fn rejects_isolated_and_curved_openings() {
        let spec = WindowSpec {
            origin: WindowOrigin {
                block_x: 0,
                block_y: 0,
            },
            profile: GridProfile::Coarse,
        };
        let (mut map, flow) = fixture(false, vec![1.]);
        map.edges[0].centerline = vec![[10., 24.], [38., 24.]];
        assert!(
            rasterize(&map, &flow, spec)
                .unwrap_err()
                .contains("no usable opening")
        );
        map.edges[0].centerline = vec![[-10., 24.], [0., 24.], [10., 25.], [58., 24.]];
        assert!(
            rasterize(&map, &flow, spec)
                .unwrap_err()
                .contains("curved opening")
        );
    }
}
