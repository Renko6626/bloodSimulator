//! Prepared world-space backgrounds. Preparation validates assets; sampling never loads chunks.
use crate::{
    Config,
    circulation::graph::{self, GraphFlow},
    map::{ChunkId, MapDefinition, fluid_policy::*},
};
use rapier2d::prelude::Vector;

/// Common depth converting graph volume flow into two-dimensional flux.
pub const EQUIVALENT_DEPTH: f32 = 1.0;

#[derive(Clone, Debug)]
pub(crate) struct PreparedBackground {
    definition: Option<MapFluidDefinition>,
    viscosity: f32,
    dt: f32,
    substeps: usize,
}

impl PreparedBackground {
    pub(crate) fn new(
        map: &MapDefinition,
        config: &Config,
        unit_flow: &GraphFlow,
    ) -> Result<Self, String> {
        let prepared = Self {
            definition: map.fluid.clone(),
            viscosity: config.viscosity,
            dt: config.dt,
            substeps: config.substeps,
        };
        // Legacy maps retain their existing drive/configuration behavior.
        let Some(definition) = &prepared.definition else {
            return Ok(prepared);
        };
        if !definition.max_base_pressure.is_finite() || definition.max_base_pressure <= 0.0 {
            return Err("background maximum base pressure must be finite and positive".into());
        }
        if !config.viscosity.is_finite()
            || config.viscosity <= 0.0
            || !config.dt.is_finite()
            || config.dt <= 0.0
            || config.substeps == 0
        {
            return Err("background needs positive viscosity, dt and substeps".into());
        }
        prepared.validate_step(config)?;
        if unit_flow.rates.len() != map.edges.len()
            || unit_flow.rates.iter().any(|q| !q.is_finite())
        {
            return Err("background needs a finite unit graph flow for every edge".into());
        }
        for policy in definition.chunks.values() {
            if let BackgroundSource::Precomputed(id) = policy.background {
                if policy.allow_lbm {
                    return Err("precomputed background cannot allow LBM".into());
                }
                if id.0 >= definition.fields.len() {
                    return Err(format!("unknown precomputed FieldId {}", id.0));
                }
            }
        }
        for (id, field) in definition.fields.iter().enumerate() {
            validate_field(map, definition, config, unit_flow, FieldId(id), field)
                .map_err(|e| format!("background field {id}: {e}"))?;
        }
        Ok(prepared)
    }

    /// Must run before advancing time, environment, entities or GPU state.
    pub(crate) fn validate_step(&self, config: &Config) -> Result<(), String> {
        let Some(definition) = &self.definition else {
            return Ok(());
        };
        if !config.pressure.is_finite()
            || config.pressure < 0.0
            || config.pressure > definition.max_base_pressure
        {
            return Err("base pressure is outside the prepared background range".into());
        }
        if config.viscosity != self.viscosity
            || config.dt != self.dt
            || config.substeps != self.substeps
        {
            return Err("background viscosity, dt or substeps changed; prepare again".into());
        }
        Ok(())
    }

    pub(crate) fn sample(
        &self,
        map: &MapDefinition,
        flow: &GraphFlow,
        instantaneous_pressure: f32,
        p: Vector,
    ) -> Vector {
        let Some(definition) = &self.definition else {
            return graph::sample_velocity(p, map, flow);
        };
        if map.signed_distance(p) >= 0.0 {
            return Vector::ZERO;
        }
        match definition.policy_at(ChunkId::at(p.to_array())).background {
            BackgroundSource::Analytic => {
                graph::sample_planar_velocity(p, map, flow, EQUIVALENT_DEPTH)
            }
            BackgroundSource::Precomputed(id) => {
                let field = &definition.fields[id.0];
                sample_field(field, p) * (instantaneous_pressure / field.reference_pressure)
            }
        }
    }
}

fn grid_coordinates(field: &PrecomputedField, p: Vector) -> Option<(usize, usize, f32, f32)> {
    let q = (p - Vector::from_array(field.origin)) / field.spacing;
    let max_x = (field.size[0] - 1) as f32;
    let max_y = (field.size[1] - 1) as f32;
    if !q.is_finite() || q.x < 0.0 || q.y < 0.0 || q.x > max_x || q.y > max_y {
        return None;
    }
    let x = (q.x.floor() as usize).min(field.size[0] - 2);
    let y = (q.y.floor() as usize).min(field.size[1] - 2);
    Some((x, y, q.x - x as f32, q.y - y as f32))
}

fn sample_field(field: &PrecomputedField, p: Vector) -> Vector {
    let Some((x, y, tx, ty)) = grid_coordinates(field, p) else {
        return Vector::ZERO;
    };
    let read = |x, y| {
        let i = y * field.size[0] + x;
        if field.fluid[i] {
            Vector::from_array(field.velocity[i])
        } else {
            Vector::ZERO
        }
    };
    read(x, y) * ((1.0 - tx) * (1.0 - ty))
        + read(x + 1, y) * (tx * (1.0 - ty))
        + read(x, y + 1) * ((1.0 - tx) * ty)
        + read(x + 1, y + 1) * (tx * ty)
}

fn validate_field(
    map: &MapDefinition,
    definition: &MapFluidDefinition,
    config: &Config,
    unit_flow: &GraphFlow,
    id: FieldId,
    field: &PrecomputedField,
) -> Result<(), String> {
    if field.geometry_revision != definition.geometry_revision
        || field.viscosity != config.viscosity
    {
        return Err("geometry revision or viscosity does not match".into());
    }
    if !field.reference_pressure.is_finite()
        || field.reference_pressure <= 0.0
        || !field.max_scale.is_finite()
        || field.max_scale < 0.0
        || !((1.15 * definition.max_base_pressure / field.reference_pressure).is_finite())
        || 1.15 * definition.max_base_pressure / field.reference_pressure > field.max_scale
    {
        return Err("reference pressure / scale cannot cover the maximum pulsating drive".into());
    }
    let count = field.size[0].checked_mul(field.size[1]);
    if field.size.iter().any(|&n| n < 2)
        || count != Some(field.velocity.len())
        || count != Some(field.fluid.len())
        || !field.spacing.is_finite()
        || field.spacing <= 0.0
        || field.origin.iter().any(|v| !v.is_finite())
        || field.velocity.iter().flatten().any(|v| !v.is_finite())
    {
        return Err("invalid finite grid, mask or velocity dimensions".into());
    }
    let end = Vector::from_array(field.origin)
        + Vector::new(
            (field.size[0] - 1) as f32 * field.spacing,
            (field.size[1] - 1) as f32 * field.spacing,
        );
    if !end.is_finite() || end.x <= field.origin[0] || end.y <= field.origin[1] {
        return Err("grid extent must be finite and positive".into());
    }
    for y in 0..field.size[1] {
        for x in 0..field.size[0] {
            let p =
                Vector::from_array(field.origin) + Vector::new(x as f32, y as f32) * field.spacing;
            let i = y * field.size[0] + x;
            if field.fluid[i] != (map.signed_distance(p) < 0.0) {
                return Err("fluid mask disagrees with authoritative map geometry".into());
            }
            if !field.fluid[i] && field.velocity[i] != [0.0, 0.0] {
                return Err("solid grid samples must have zero velocity".into());
            }
        }
    }
    for port in &field.ports {
        validate_port(map, unit_flow, field, port)?;
        let center = map.edge_point(port.edge, port.along, 0.0);
        let outward = map.project_edge(port.edge, center).tangent * port.outward_sign;
        let epsilon = field.spacing * 0.01;
        let inside = definition.policy_at(ChunkId::at((center - outward * epsilon).to_array()));
        let outside = definition.policy_at(ChunkId::at((center + outward * epsilon).to_array()));
        if inside.background != BackgroundSource::Precomputed(id)
            || outside.background != BackgroundSource::Analytic
        {
            return Err("port must connect its field outward to analytic background".into());
        }
    }

    // Check the source region, not display tiles or the support-data rectangle.
    // Face samples are at most h/2 apart; a valid port has >=8 grid intervals across
    // its diameter (see validate_port). No boundary tracing/generation is needed.
    for (&chunk, policy) in &definition.chunks {
        if policy.background != BackgroundSource::Precomputed(id) {
            continue;
        }
        let [min, max] = chunk.bounds();
        let steps = ((max[0] - min[0]) / (field.spacing * 0.5)).ceil().max(1.0) as usize;
        // Conservative support bounds: each tube segment's radius-expanded AABB
        // is clipped to the labelled chunk. This can ask for extra solid samples
        // near diagonal channels, but cannot miss fluid between grid probes.
        for edge in &map.edges {
            for segment in edge.centerline.windows(2) {
                let low = [
                    (segment[0][0].min(segment[1][0]) - edge.half_width).max(min[0]),
                    (segment[0][1].min(segment[1][1]) - edge.half_width).max(min[1]),
                ];
                let high = [
                    (segment[0][0].max(segment[1][0]) + edge.half_width).min(max[0]),
                    (segment[0][1].max(segment[1][1]) + edge.half_width).min(max[1]),
                ];
                if low[0] <= high[0]
                    && low[1] <= high[1]
                    && (grid_coordinates(field, Vector::from_array(low)).is_none()
                        || grid_coordinates(field, Vector::from_array(high)).is_none())
                {
                    return Err(
                        "asset does not cover labelled fluid and interpolation support".into(),
                    );
                }
            }
        }
        for (delta, a, b, normal) in [
            (
                (-1, 0),
                Vector::new(min[0], min[1]),
                Vector::new(min[0], max[1]),
                Vector::NEG_X,
            ),
            (
                (1, 0),
                Vector::new(max[0], min[1]),
                Vector::new(max[0], max[1]),
                Vector::X,
            ),
            (
                (0, -1),
                Vector::new(min[0], min[1]),
                Vector::new(max[0], min[1]),
                Vector::NEG_Y,
            ),
            (
                (0, 1),
                Vector::new(min[0], max[1]),
                Vector::new(max[0], max[1]),
                Vector::Y,
            ),
        ] {
            let neighbor = ChunkId {
                x: chunk.x + delta.0,
                y: chunk.y + delta.1,
            };
            let source = definition.policy_at(neighbor).background;
            if source == BackgroundSource::Precomputed(id) {
                continue;
            }
            for i in 0..=steps {
                let p = a.lerp(b, i as f32 / steps as f32);
                if map.signed_distance(p) >= 0.0 {
                    continue;
                }
                if source != BackgroundSource::Analytic {
                    return Err("different static fields may not meet in fluid".into());
                }
                let port = field
                    .ports
                    .iter()
                    .find(|port| port_matches_boundary(map, field, port, p, normal))
                    .ok_or("fluid source boundary lacks a declared straight port")?;
                check_port_velocity(map, unit_flow, field, port, p)?;
            }
        }
    }
    Ok(())
}

fn port_matches_boundary(
    map: &MapDefinition,
    field: &PrecomputedField,
    port: &FieldPort,
    p: Vector,
    normal: Vector,
) -> bool {
    let edge = &map.edges[port.edge];
    let projection = map.project_edge(port.edge, p);
    let center = map.edge_point(port.edge, port.along, 0.0);
    let tangent = map.project_edge(port.edge, center).tangent;
    projection.distance < edge.half_width
        && (projection.along - port.along).abs() <= edge.half_width + field.spacing
        && tangent.dot(normal) * port.outward_sign > 0.1
}

fn validate_port(
    map: &MapDefinition,
    unit_flow: &GraphFlow,
    field: &PrecomputedField,
    port: &FieldPort,
) -> Result<(), String> {
    let edge = map
        .edges
        .get(port.edge)
        .ok_or("port refers to unknown graph edge")?;
    if !port.along.is_finite()
        || port.along < 0.0
        || port.along > map.edge_length(port.edge)
        || !matches!(port.outward_sign, -1.0 | 1.0)
        || !port.reference_flux.is_finite()
    {
        return Err("invalid port position, direction or reference flux".into());
    }
    let expected = unit_flow.rates[port.edge] * field.reference_pressure * port.outward_sign
        / EQUIVALENT_DEPTH;
    if !expected.is_finite()
        || (port.reference_flux - expected).abs() > expected.abs() * 1e-5 + 1e-6
    {
        return Err("port reference flux/direction does not match the common graph drive".into());
    }
    let radius = edge.half_width;
    if field.spacing > radius * 0.25 {
        return Err("port needs at least eight grid intervals across its diameter".into());
    }
    // Keep a full radius away from vertices/junctions. A straight segment may be
    // subdivided without changing geometry, so test centerline directions locally.
    let center = map.edge_point(port.edge, port.along, 0.0);
    let tangent = map.project_edge(port.edge, center).tangent;
    if port.along < radius * 2.0 || map.edge_length(port.edge) - port.along < radius * 2.0 {
        return Err("port is too close to a graph junction".into());
    }
    let before = map.edge_point(port.edge, port.along - radius * 2.0, 0.0);
    let after = map.edge_point(port.edge, port.along + radius * 2.0, 0.0);
    if (before - (center - tangent * (radius * 2.0))).length() > 1e-4
        || (after - (center + tangent * (radius * 2.0))).length() > 1e-4
    {
        return Err("port must lie in a straight pipe".into());
    }
    let normal = Vector::new(-tangent.y, tangent.x);
    let intervals = ((radius * 2.0 / field.spacing).ceil() as usize * 4).max(16);
    let width = radius * 2.0 / intervals as f32;
    let mut flux = 0.0;
    for i in 0..=intervals {
        let p = center + normal * (-radius + i as f32 * width);
        if grid_coordinates(field, p).is_none() {
            return Err("asset lacks port interpolation support".into());
        }
        check_port_velocity(map, unit_flow, field, port, p)?;
        flux += sample_field(field, p).dot(tangent)
            * port.outward_sign
            * width
            * if i == 0 || i == intervals { 0.5 } else { 1.0 };
    }
    if (flux - expected).abs() > port_tolerance(field, radius) * expected.abs() + 1e-6 {
        return Err("port integrated planar flux disagrees with graph flux".into());
    }
    Ok(())
}

// Linear interpolation of a parabola has error O((h/R)^2). This allows that
// discretization error plus f32 rounding; coarse assets are rejected above.
fn port_tolerance(field: &PrecomputedField, radius: f32) -> f32 {
    (field.spacing / radius).powi(2) + 1e-4
}

fn check_port_velocity(
    map: &MapDefinition,
    unit_flow: &GraphFlow,
    field: &PrecomputedField,
    port: &FieldPort,
    p: Vector,
) -> Result<(), String> {
    let radius = map.edges[port.edge].half_width;
    let projection = map.project_edge(port.edge, p);
    let peak = 3.0 * unit_flow.rates[port.edge] * field.reference_pressure
        / (4.0 * radius * EQUIVALENT_DEPTH);
    let expected =
        projection.tangent * peak * (1.0 - (projection.distance / radius).powi(2)).max(0.0);
    if (sample_field(field, p) - expected).length()
        > peak.abs() * port_tolerance(field, radius) + 1e-6
    {
        return Err("static port velocity profile does not match planar analytic flow".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{circulation::graph::GraphNetwork, map::VesselEdge};
    use std::collections::BTreeMap;

    fn fixture() -> (MapDefinition, Config, GraphFlow) {
        let nodes = vec![[-32.0, 8.0], [64.0, 8.0], [64.0, 40.0], [-32.0, 40.0]];
        let edges = (0..4)
            .map(|from| {
                let to = (from + 1) % 4;
                VesselEdge {
                    from,
                    to,
                    centerline: vec![nodes[from], nodes[to]],
                    half_width: 2.0,
                }
            })
            .collect();
        let mut map = MapDefinition {
            fluid: None,
            nodes,
            edges,
            pump_edge: 0,
            spawn: [16.0, 8.0],
            landmarks: vec![],
        };
        let config = Config {
            pressure: 1.0,
            ..Config::default()
        };
        let unit = GraphNetwork::new(&map, config.viscosity).solve(1.0);
        let mut field = PrecomputedField {
            geometry_revision: 7,
            viscosity: config.viscosity,
            reference_pressure: 1.0,
            max_scale: 2.3,
            origin: [-0.25, 5.75],
            spacing: 0.25,
            size: [131, 19],
            velocity: vec![],
            fluid: vec![],
            ports: vec![
                FieldPort {
                    edge: 0,
                    along: 32.0,
                    outward_sign: -1.0,
                    reference_flux: -unit.rates[0],
                },
                FieldPort {
                    edge: 0,
                    along: 64.0,
                    outward_sign: 1.0,
                    reference_flux: unit.rates[0],
                },
            ],
        };
        for y in 0..field.size[1] {
            for x in 0..field.size[0] {
                let p = Vector::from_array(field.origin)
                    + Vector::new(x as f32, y as f32) * field.spacing;
                field.fluid.push(map.signed_distance(p) < 0.0);
                field.velocity.push(
                    graph::sample_planar_velocity(p, &map, &unit, EQUIVALENT_DEPTH).to_array(),
                );
            }
        }
        map.fluid = Some(MapFluidDefinition {
            geometry_revision: 7,
            max_base_pressure: 2.0,
            chunks: BTreeMap::from([
                (
                    ChunkId { x: 0, y: 0 },
                    ChunkFluidPolicy {
                        background: BackgroundSource::Precomputed(FieldId(0)),
                        allow_lbm: false,
                    },
                ),
                (
                    ChunkId { x: 1, y: 0 },
                    ChunkFluidPolicy {
                        background: BackgroundSource::Precomputed(FieldId(0)),
                        allow_lbm: false,
                    },
                ),
            ]),
            fields: vec![field],
        });
        (map, config, unit)
    }

    #[test]
    fn background_planar_pipe_preserves_cross_section_flux() {
        let (map, _, mut flow) = fixture();
        flow.rates[0] = 8.0;
        for (y, expected) in [(8.0, 3.0), (9.0, 2.25), (10.0, 0.0)] {
            let velocity = graph::sample_planar_velocity(Vector::new(16.0, y), &map, &flow, 1.0);
            assert!((velocity.x - expected).abs() < 1e-6);
            assert_eq!(velocity.y, 0.0);
        }
        let n = 1000;
        let flux: f32 = (0..n)
            .map(|i| {
                let y = 6.0 + (i as f32 + 0.5) * 4.0 / n as f32;
                graph::sample_planar_velocity(Vector::new(16.0, y), &map, &flow, 1.0).x * 4.0
                    / n as f32
            })
            .sum();
        assert!((flux - 8.0).abs() < 1e-4, "flux {flux}");
    }

    #[test]
    fn background_bilinear_midpoint_and_common_pressure_scale() {
        let (mut map, config, flow) = fixture();
        let field = &mut map.fluid.as_mut().unwrap().fields[0];
        field.origin = [15.0, 7.0];
        field.spacing = 2.0;
        field.size = [2, 2];
        field.fluid = vec![true; 4];
        field.velocity = vec![[0.0, 0.0], [2.0, 0.0], [0.0, 4.0], [2.0, 4.0]];
        field.reference_pressure = 2.0;
        // Isolate sampling from preparation: this tiny table is intentionally not
        // a full region asset, and its expected midpoint is independent of the pipe.
        let prepared = PreparedBackground {
            definition: map.fluid.clone(),
            viscosity: config.viscosity,
            dt: config.dt,
            substeps: config.substeps,
        };
        assert_eq!(
            prepared.sample(&map, &flow, 2.0, Vector::new(16.0, 8.0)),
            Vector::new(1.0, 2.0)
        );
        assert_eq!(
            prepared.sample(&map, &flow, 1.0, Vector::new(16.0, 8.0)),
            Vector::new(0.5, 1.0)
        );
        assert_eq!(
            prepared.sample(&map, &flow, 2.0, Vector::new(16.0, 12.0)),
            Vector::ZERO
        );
    }

    #[test]
    fn background_shared_asset_is_continuous_at_chunk_boundary() {
        let (map, config, flow) = fixture();
        let prepared = PreparedBackground::new(&map, &config, &flow).unwrap();
        let left = prepared.sample(&map, &flow, 1.0, Vector::new(16.0 - 1e-3, 8.0));
        let right = prepared.sample(&map, &flow, 1.0, Vector::new(16.0 + 1e-3, 8.0));
        assert!((left - right).length() < 1e-6);
        let expected = 3.0 * flow.rates[0] / 8.0;
        assert!((left.x - expected).abs() < 1e-6);
        assert_eq!(
            prepared.sample(&map, &flow, 0.0, Vector::new(16.0, 8.0)),
            Vector::ZERO
        );
    }

    #[test]
    fn background_rejects_wrong_reference_flux_and_unprepared_drive() {
        let (mut map, config, flow) = fixture();
        let prepared = PreparedBackground::new(&map, &config, &flow).unwrap();
        for pressure in [-0.1, 2.01, f32::NAN] {
            let changed = Config {
                pressure,
                ..config.clone()
            };
            assert!(prepared.validate_step(&changed).is_err());
        }
        for changed in [
            Config {
                viscosity: config.viscosity * 2.0,
                ..config.clone()
            },
            Config {
                dt: config.dt * 2.0,
                ..config.clone()
            },
            Config {
                substeps: config.substeps + 1,
                ..config.clone()
            },
        ] {
            assert!(prepared.validate_step(&changed).is_err());
        }
        map.fluid.as_mut().unwrap().fields[0].ports[1].reference_flux *= 1.5;
        assert!(
            PreparedBackground::new(&map, &config, &flow)
                .err()
                .unwrap()
                .contains("reference flux")
        );
    }

    #[test]
    fn background_rejects_missing_ports_bad_mask_and_unsafe_policy() {
        let (map, config, flow) = fixture();
        let mut broken = map.clone();
        broken.fluid.as_mut().unwrap().max_base_pressure = 0.;
        assert!(PreparedBackground::new(&broken, &config, &flow).is_err());
        let mut broken = map.clone();
        broken.fluid.as_mut().unwrap().fields[0].ports.clear();
        assert!(
            PreparedBackground::new(&broken, &config, &flow)
                .err()
                .unwrap()
                .contains("declared straight port")
        );
        let mut broken = map.clone();
        for velocity in &mut broken.fluid.as_mut().unwrap().fields[0].velocity {
            velocity[0] *= 1.5;
        }
        assert!(
            PreparedBackground::new(&broken, &config, &flow)
                .err()
                .unwrap()
                .contains("velocity profile")
        );
        let mut broken = map.clone();
        broken.fluid.as_mut().unwrap().fields[0].max_scale = 2.0;
        assert!(
            PreparedBackground::new(&broken, &config, &flow)
                .err()
                .unwrap()
                .contains("maximum pulsating drive")
        );
        let mut broken = map.clone();
        broken.fluid.as_mut().unwrap().fields[0].fluid[0] = true;
        assert!(
            PreparedBackground::new(&broken, &config, &flow)
                .err()
                .unwrap()
                .contains("mask")
        );
        let mut broken = map.clone();
        broken
            .fluid
            .as_mut()
            .unwrap()
            .chunks
            .values_mut()
            .next()
            .unwrap()
            .allow_lbm = true;
        assert!(
            PreparedBackground::new(&broken, &config, &flow)
                .err()
                .unwrap()
                .contains("allow LBM")
        );
        let mut broken = map.clone();
        broken
            .fluid
            .as_mut()
            .unwrap()
            .chunks
            .values_mut()
            .next()
            .unwrap()
            .background = BackgroundSource::Precomputed(FieldId(9));
        assert!(
            PreparedBackground::new(&broken, &config, &flow)
                .err()
                .unwrap()
                .contains("unknown")
        );
    }
}
