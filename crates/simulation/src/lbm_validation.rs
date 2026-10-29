//! Explicit playable fixture and bounded input replay; never selected by default.
use crate::{
    Config, MoveInput, Snapshot,
    circulation::graph::GraphNetwork,
    map::{Landmark, MapDefinition, VesselEdge},
};
use rapier2d::prelude::Vector;

pub fn map() -> MapDefinition {
    let nodes = vec![
        [-50., 24.],
        [24., 24.],
        [100., 62.],
        [-14., 100.],
        [100., 140.],
        [-50., 140.],
    ];
    let pairs = [(0, 1), (1, 2), (1, 3), (2, 4), (3, 4), (4, 5), (5, 0)];
    let edges = pairs
        .into_iter()
        .map(|(from, to)| VesselEdge {
            from,
            to,
            half_width: 3.,
            centerline: if (from, to) == (3, 4) {
                vec![nodes[from], [60., 100.], nodes[to]]
            } else {
                vec![nodes[from], nodes[to]]
            },
        })
        .collect();
    MapDefinition {
        nodes,
        edges,
        pump_edge: 6,
        spawn: [24., 24.],
        landmarks: vec![Landmark {
            name: "LBM 分叉验证场".into(),
            position: [24., 13.],
            size: [20., 5.],
        }],
    }
}
/// Calibration is an explicit initialization choice. Reset reuses current config.
pub fn config() -> Config {
    let mut config = Config::default();
    let unit = GraphNetwork::new(&map(), config.viscosity).solve(1.);
    config.pressure = 18.529297 / unit.summary.rate;
    config
}
pub(crate) fn centers(map: &MapDefinition, config: &Config) -> Vec<Vector> {
    if config.cell_count == 0 {
        return Vec::new();
    }
    let spawn = Vector::from_array(map.spawn);
    let spacing = 2. * config.cell_radius + 0.3;
    let mut candidates = Vec::new();
    for edge in 0..3 {
        let length = map.edge_length(edge);
        for k in 0..24 {
            let along = if edge == 0 {
                length - 2. - k as f32 * spacing
            } else {
                2. + k as f32 * spacing
            };
            for lane in [-1., 0., 1.] {
                let p = map.edge_point(edge, along, lane * 1.75);
                if map.signed_distance(p) < -config.cell_radius - 0.16
                    && (p - spawn).length() > config.cell_radius + 0.85 + 0.5
                {
                    candidates.push(p);
                }
            }
        }
    }
    candidates.sort_by(|a, b| {
        (*a - spawn)
            .length_squared()
            .total_cmp(&(*b - spawn).length_squared())
    });
    let mut result: Vec<Vector> = Vec::new();
    for p in candidates {
        if result.iter().all(|q| (*q - p).length() >= spacing - 1e-5) {
            result.push(p);
        }
        if result.len() == config.cell_count {
            break;
        }
    }
    assert_eq!(
        result.len(),
        config.cell_count,
        "validation seed has insufficient room"
    );
    result
}
/// Position feedback through the normal drive, never teleportation. Each 12 s
/// leg allows contact slowdown; actual migration is checked from observations.
pub fn movement(moving: bool, frame: usize, warmup: usize, position: [f32; 2]) -> MoveInput {
    let target = if !moving || frame < warmup {
        [24., 24.]
    } else if ((frame - warmup) / 720) % 2 == 0 {
        [10., 24.]
    } else {
        [28., 24.]
    };
    let d = Vector::from_array(target) - Vector::from_array(position);
    let u = (d * 2.).clamp_length_max(1.);
    MoveInput { x: u.x, y: u.y }
}
#[derive(Clone, Copy, Debug)]
pub struct Quality {
    pub cell_count: usize,
    pub finite: bool,
    pub area_ratio_min: f32,
    pub area_ratio_max: f32,
    pub node_wall_max: f32,
    pub robot_wall: f32,
    pub nearby_cells: usize,
    pub cell_displacement_mean: f32,
    pub cell_displacement_max: f32,
}
pub fn quality(
    snapshot: &Snapshot,
    map: &MapDefinition,
    config: &Config,
    initial: &[[f32; 2]],
) -> Quality {
    let rest = 0.5
        * config.cell_vertices as f32
        * config.cell_radius.powi(2)
        * (std::f32::consts::TAU / config.cell_vertices as f32).sin();
    let robot = Vector::from_array(snapshot.robot);
    let mut result = Quality {
        cell_count: snapshot.cells.len(),
        finite: robot.is_finite(),
        area_ratio_min: f32::INFINITY,
        area_ratio_max: 0.,
        node_wall_max: f32::NEG_INFINITY,
        robot_wall: map.signed_distance(robot),
        nearby_cells: 0,
        cell_displacement_mean: 0.,
        cell_displacement_max: 0.,
    };
    for (index, cell) in snapshot.cells.iter().enumerate() {
        let mut center = Vector::ZERO;
        let mut area = 0.;
        for (i, p) in cell.iter().enumerate() {
            let p = Vector::from_array(*p);
            result.finite &= p.is_finite();
            center += p;
            area += p.perp_dot(Vector::from_array(cell[(i + 1) % cell.len()]));
            result.node_wall_max = result.node_wall_max.max(map.signed_distance(p));
        }
        center /= cell.len() as f32;
        if (center - robot).length() <= 8. {
            result.nearby_cells += 1;
        }
        let ratio = area.abs() * 0.5 / rest;
        result.area_ratio_min = result.area_ratio_min.min(ratio);
        result.area_ratio_max = result.area_ratio_max.max(ratio);
        if let Some(p) = initial.get(index) {
            let d = (center - Vector::from_array(*p)).length();
            result.cell_displacement_mean += d;
            result.cell_displacement_max = result.cell_displacement_max.max(d);
        }
    }
    result.cell_displacement_mean /= snapshot.cells.len().max(1) as f32;
    result
}
pub fn cell_centers(snapshot: &Snapshot) -> Vec<[f32; 2]> {
    snapshot
        .cells
        .iter()
        .map(|cell| {
            let p = cell
                .iter()
                .fold(Vector::ZERO, |a, p| a + Vector::from_array(*p))
                / cell.len() as f32;
            p.to_array()
        })
        .collect()
}
pub fn guard(q: Quality) -> Result<(), String> {
    if !q.finite
        || (q.cell_count > 0
            && (!q.area_ratio_min.is_finite()
                || q.area_ratio_min < 0.5
                || q.area_ratio_max > 1.5
                || q.node_wall_max > 0.3))
        || q.robot_wall > -0.55
    {
        return Err(format!("invalid validation mechanics: {q:?}"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_detailed_window_checks_robot_without_claiming_cell_quality() {
        let config = Config {
            cell_count: 0,
            ..Default::default()
        };
        let sim = crate::Simulation::new_world(config.clone());
        let mut snapshot = sim.snapshot();
        let q = quality(&snapshot, sim.world_map().unwrap(), &config, &[]);
        assert_eq!(q.cell_count, 0);
        assert!(guard(q).is_ok());
        snapshot.robot[0] = f32::NAN;
        assert!(guard(quality(&snapshot, sim.world_map().unwrap(), &config, &[])).is_err());
    }

    #[test]
    #[ignore = "explicit software-adapter validation-scene reset check"]
    fn validation_reset_preserves_seed_and_current_pressure() {
        let config = config();
        let mut sim = crate::Simulation::new_lbm_validation(
            config,
            crate::LbmOptions {
                allow_software: true,
                ..Default::default()
            },
        )
        .unwrap();
        let original = sim.snapshot().cells;
        sim.config.pressure *= 0.8;
        let pressure = sim.config.pressure;
        sim.set_paused(true);
        sim.try_reset().unwrap();
        let restored = sim.snapshot();
        assert_eq!(restored.cells, original);
        assert_eq!(sim.config.pressure, pressure);
        assert!(restored.paused);
        assert_eq!(restored.cells.len(), 72);
        assert_eq!(sim.world_map().unwrap().pump_edge, 6);
    }
    #[test]
    fn graph_flow_and_local_seed_are_explicit() {
        let map = map();
        let config = config();
        let flow = GraphNetwork::new(&map, config.viscosity).solve(config.pressure);
        // Separate physical tubes on the far return; shared graph endpoint only.
        for fraction in [0.25, 0.5, 0.75] {
            let p = map.edge_point(4, map.edge_length(4) * fraction, 0.);
            assert!(map.project_edge(5, p).distance > 6.);
        }
        assert!((flow.rates[0] - 18.529297).abs() < 1e-4);
        assert!((flow.rates[0] - flow.rates[1] - flow.rates[2]).abs() < 1e-4);
        let points = centers(&map, &config);
        assert_eq!(points.len(), 72);
        for (i, p) in points.iter().enumerate() {
            assert!(map.signed_distance(*p) < -0.86);
            assert!(points[..i].iter().all(|q| (*q - *p).length() > 1.69));
            assert!((*p - Vector::from_array(map.spawn)).length() < 26.);
        }
        assert!(movement(true, 120, 120, [24., 24.]).x < 0.);
        assert!(movement(true, 840, 120, [10., 24.]).x > 0.);
    }
}
