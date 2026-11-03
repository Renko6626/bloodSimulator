//! Conversion only: normal map/background validation remains required on import.
use crate::map::{
    fluid_asset::validate_structure,
    fluid_policy::{FieldPort, PrecomputedField},
};
use blood_fluid_gpu::lbm::bake::BakedFlow;

pub fn to_precomputed_field(
    baked: &BakedFlow,
    geometry_revision: u64,
    reference_pressure: f32,
    max_scale: f32,
    ports: Vec<FieldPort>,
) -> Result<PrecomputedField, String> {
    baked.domain.validate()?;
    let side = usize::try_from(baked.domain.cells_per_side)
        .map_err(|_| "baked dimension exceeds usize")?;
    let field = PrecomputedField {
        geometry_revision,
        viscosity: baked.viscosity,
        reference_pressure,
        max_scale,
        origin: baked.domain.origin.map(|x| x + baked.domain.spacing * 0.5),
        spacing: baked.domain.spacing,
        size: [side; 2],
        velocity: baked.velocity.clone(),
        fluid: baked.fluid.clone(),
        ports,
    };
    validate_structure(&field)?;
    Ok(field)
}

/// Resample actual field velocities onto a map-aligned export grid.
/// All target samples must have source interpolation support, including solids.
/// The map determines the new mask; wall/outside samples are exactly zero.
pub fn resample_precomputed_field(
    map: &crate::map::MapDefinition,
    source: &PrecomputedField,
    origin: [f32; 2],
    spacing: f32,
    size: [usize; 2],
) -> Result<PrecomputedField, String> {
    use crate::background::{grid_coordinates, sample_field};
    use rapier2d::prelude::Vector;
    validate_structure(source)?;
    let count = size[0]
        .checked_mul(size[1])
        .ok_or("resampled size overflow")?;
    if size.iter().any(|&n| n < 2)
        || !spacing.is_finite()
        || spacing <= 0.
        || origin.iter().any(|v| !v.is_finite())
    {
        return Err("invalid resampled grid".into());
    }
    let end = Vector::from_array(origin)
        + Vector::new((size[0] - 1) as f32, (size[1] - 1) as f32) * spacing;
    if !end.is_finite()
        || end.x <= origin[0]
        || end.y <= origin[1]
        || grid_coordinates(source, Vector::from_array(origin)).is_none()
        || grid_coordinates(source, end).is_none()
    {
        return Err("resampled grid lacks source interpolation support".into());
    }
    let mut result = PrecomputedField {
        origin,
        spacing,
        size,
        velocity: Vec::with_capacity(count),
        fluid: Vec::with_capacity(count),
        ..source.clone()
    };
    for y in 0..size[1] {
        for x in 0..size[0] {
            let point = Vector::from_array(origin) + Vector::new(x as f32, y as f32) * spacing;
            let fluid = map.signed_distance(point) < 0.;
            result.fluid.push(fluid);
            result.velocity.push(if fluid {
                sample_field(source, point).to_array()
            } else {
                [0.; 2]
            });
        }
    }
    validate_structure(&result)?;
    Ok(result)
}

/// Export known boundary traces and one exterior support row. Interior fluid
/// nodes only interpolate the actual solver field; no interior blend is applied.
pub fn to_boundary_aligned_field(
    map: &crate::map::MapDefinition,
    flow: &crate::circulation::graph::GraphFlow,
    baked: &BakedFlow,
    geometry_revision: u64,
    reference_pressure: f32,
    max_scale: f32,
    ports: Vec<FieldPort>,
) -> Result<PrecomputedField, String> {
    use crate::{
        background::{EQUIVALENT_DEPTH, grid_coordinates, sample_field},
        circulation::graph,
    };
    use rapier2d::prelude::Vector;
    let source = to_precomputed_field(
        baked,
        geometry_revision,
        reference_pressure,
        max_scale,
        ports,
    )?;
    let [low, high] = baked.domain.world_bounds().map(Vector::from_array);
    let h = baked.domain.spacing;
    let side = baked.domain.cells_per_side as usize + 3;
    let mut result = PrecomputedField {
        origin: (low - Vector::splat(h)).to_array(),
        size: [side; 2],
        velocity: Vec::with_capacity(side * side),
        fluid: Vec::with_capacity(side * side),
        ..source.clone()
    };
    for y in 0..side {
        for x in 0..side {
            let point = Vector::from_array(result.origin) + Vector::new(x as f32, y as f32) * h;
            let fluid = map.signed_distance(point) < 0.;
            let velocity = if !fluid {
                Vector::ZERO
            } else if point.x > low.x && point.x < high.x && point.y > low.y && point.y < high.y {
                if grid_coordinates(&source, point).is_none() {
                    return Err("interior export node lacks actual solver support".into());
                }
                sample_field(&source, point)
            } else {
                let declared = source.ports.iter().any(|port| {
                    let Some(edge) = map.edges.get(port.edge) else {
                        return false;
                    };
                    let center = map.edge_point(port.edge, port.along, 0.);
                    let tangent = map.project_edge(port.edge, center).tangent;
                    let outward = tangent * port.outward_sign;
                    let projection = map.project_edge(port.edge, point);
                    let on_face = [
                        (center.x - low.x, outward.x * -1.),
                        (center.x - high.x, outward.x),
                        (center.y - low.y, outward.y * -1.),
                        (center.y - high.y, outward.y),
                    ]
                    .into_iter()
                    .any(|(distance, dot)| distance.abs() <= h * 1e-3 && dot > 0.1);
                    on_face
                        && projection.distance < edge.half_width
                        && (projection.along - port.along).abs() <= edge.half_width + 2. * h
                });
                if !declared {
                    return Err(format!("undeclared boundary/support fluid at {point:?}"));
                }
                graph::sample_planar_velocity(point, map, flow, EQUIVALENT_DEPTH)
            };
            result.fluid.push(fluid);
            result.velocity.push(velocity.to_array());
        }
    }
    validate_structure(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use blood_fluid_gpu::lbm::{
        Diagnostics, DomainSpec,
        bake::{BakeReport, BakeSchedule},
    };
    fn baked_fixture() -> BakedFlow {
        BakedFlow {
            domain: DomainSpec {
                origin: [-1., 3.],
                cells_per_side: 2,
                spacing: 0.5,
            },
            viscosity: 0.125,
            velocity: vec![[1., 2.], [0., 0.], [3., 4.], [5., 6.]],
            fluid: vec![true, false, true, true],
            report: BakeReport {
                schedule: BakeSchedule {
                    steps_per_frame: 1,
                    warmup_frames: 0,
                    sample_frames: 2,
                    sample_stride: 1,
                },
                lattice_dt: 0.01,
                samples: 2,
                warmup_simulated_seconds: 0.,
                sample_simulated_seconds: 0.02,
                wall_seconds: 0.,
                density_min: 1.,
                density_max: 1.,
                mean_opening_fluxes: vec![],
                half_mean_velocity_rms: 0.,
                half_mean_velocity_max: 0.,
                final_diagnostics: Diagnostics {
                    density_min: 1.,
                    density_max: 1.,
                    max_speed: 1.,
                    mass: 3.,
                    opening_fluxes: vec![],
                    software_adapter: false,
                    adapter_name: String::new(),
                },
            },
        }
    }
    #[test]
    fn conversion_preserves_centers_row_order_mask_and_solver_viscosity() {
        let baked = baked_fixture();
        let field = to_precomputed_field(&baked, 7, 2., 3., vec![]).unwrap();
        assert_eq!(field.origin, [-0.75, 3.25]);
        assert_eq!(field.spacing, 0.5);
        assert_eq!(field.size, [2, 2]);
        assert_eq!(field.velocity, baked.velocity);
        assert_eq!(field.fluid, baked.fluid);
        assert_eq!(field.viscosity, 0.125);
        assert_eq!(
            (
                field.geometry_revision,
                field.reference_pressure,
                field.max_scale
            ),
            (7, 2., 3.)
        );
    }
    #[test]
    fn resampling_uses_bilinear_velocity_map_walls_and_checked_support() {
        use crate::map::{MapDefinition, VesselEdge};
        let map = MapDefinition {
            fluid: None,
            chambers: vec![],
            nodes: vec![[-10., 0.], [10., 0.]],
            edges: vec![VesselEdge {
                from: 0,
                to: 1,
                half_width: 1.,
                centerline: vec![[-10., 0.], [10., 0.]],
            }],
            pump_edge: 0,
            spawn: [0.; 2],
            landmarks: vec![],
        };
        let source = PrecomputedField {
            geometry_revision: 7,
            viscosity: 0.05,
            reference_pressure: 2.,
            max_scale: 3.,
            origin: [-1.5; 2],
            spacing: 1.,
            size: [4; 2],
            ports: vec![],
            fluid: (0..4)
                .flat_map(|y| (0..4).map(move |_| y == 1 || y == 2))
                .collect(),
            velocity: (0..4)
                .flat_map(|y| {
                    (0..4).map(move |x| {
                        if y == 1 || y == 2 {
                            [x as f32 + 2. * y as f32, 0.]
                        } else {
                            [0.; 2]
                        }
                    })
                })
                .collect(),
        };
        let result = resample_precomputed_field(&map, &source, [-1.; 2], 1., [3; 2]).unwrap();
        assert_eq!(
            result.fluid,
            [false, false, false, true, true, true, false, false, false]
        );
        assert_eq!(
            result.velocity,
            [
                [0.; 2],
                [0.; 2],
                [0.; 2],
                [3.5, 0.],
                [4.5, 0.],
                [5.5, 0.],
                [0.; 2],
                [0.; 2],
                [0.; 2]
            ]
        );
        assert_eq!(
            (
                result.geometry_revision,
                result.viscosity,
                result.reference_pressure,
                result.max_scale
            ),
            (7, 0.05, 2., 3.)
        );
        assert!(resample_precomputed_field(&map, &source, [-2., -1.], 1., [3; 2]).is_err());
    }
    #[test]
    fn boundary_export_keeps_actual_interior_and_prescribed_trace() {
        use crate::{
            background::sample_field,
            circulation::{Flow, graph::GraphFlow},
            map::{MapDefinition, VesselEdge},
        };
        use rapier2d::prelude::Vector;
        let map = MapDefinition {
            fluid: None,
            chambers: vec![],
            nodes: vec![[-8., 0.], [8., 0.]],
            edges: vec![VesselEdge {
                from: 0,
                to: 1,
                half_width: 1.,
                centerline: vec![[-8., 0.], [8., 0.]],
            }],
            pump_edge: 0,
            spawn: [2., 0.],
            landmarks: vec![],
        };
        let flow = GraphFlow {
            pressures: vec![],
            rates: vec![2.],
            summary: Flow {
                pressure_drop: 2.,
                resistance: 1.,
                rate: 2.,
                mean_speed: 1.,
            },
        };
        let mut baked = baked_fixture();
        baked.domain = DomainSpec {
            origin: [0., -2.],
            cells_per_side: 16,
            spacing: 0.25,
        };
        baked.fluid = (0..16)
            .flat_map(|y| (0..16).map(move |x| map_point(x, y)))
            .collect();
        fn map_point(_x: usize, y: usize) -> bool {
            ((y as f32 + 0.5) * 0.25 - 2.).abs() < 1.
        }
        baked.velocity = baked
            .fluid
            .iter()
            .map(|&f| if f { [0.75, 0.125] } else { [0.; 2] })
            .collect();
        let ports = vec![
            FieldPort {
                edge: 0,
                along: 8.,
                outward_sign: -1.,
                reference_flux: -2.,
            },
            FieldPort {
                edge: 0,
                along: 12.,
                outward_sign: 1.,
                reference_flux: 2.,
            },
        ];
        let field =
            to_boundary_aligned_field(&map, &flow, &baked, 7, 2., 1.15, ports.clone()).unwrap();
        assert_eq!(
            sample_field(&field, Vector::new(0., 0.)),
            Vector::new(1.5, 0.)
        );
        assert_eq!(
            sample_field(&field, Vector::new(-0.25, 0.)),
            Vector::new(1.5, 0.)
        );
        assert_eq!(
            sample_field(&field, Vector::new(2., 0.)),
            Vector::new(0.75, 0.125)
        );
        assert_eq!(sample_field(&field, Vector::new(0., 1.)), Vector::ZERO);
        assert!(
            to_boundary_aligned_field(&map, &flow, &baked, 7, 2., 1.15, ports[..1].to_vec())
                .unwrap_err()
                .contains("undeclared")
        );
    }
}
