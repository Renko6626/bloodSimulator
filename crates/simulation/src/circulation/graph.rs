//! A small, fixed resistance graph and a local velocity approximation for its tubes.
use super::Flow;
use crate::map::MapDefinition;
use rapier2d::prelude::Vector;
use std::f64::consts::PI;

pub struct GraphNetwork {
    unit_pressures: Vec<f64>,
    unit_rates: Vec<f64>,
    pump_edge: usize,
    pump_area: f64,
}

#[derive(Clone, Debug)]
pub struct GraphFlow {
    pub pressures: Vec<f32>,
    pub rates: Vec<f32>,
    pub summary: Flow,
}

impl GraphNetwork {
    pub fn new(map: &MapDefinition, viscosity: f32) -> Self {
        assert!(
            viscosity.is_finite() && viscosity > 0.0,
            "viscosity must be positive and finite"
        );
        assert!(!map.nodes.is_empty(), "flow graph needs at least one node");
        assert!(!map.edges.is_empty(), "flow graph needs at least one edge");
        assert!(map.pump_edge < map.edges.len(), "pump edge is out of range");
        let n = map.nodes.len();
        let mut ends = Vec::with_capacity(map.edges.len());
        let mut conductances = Vec::with_capacity(map.edges.len());
        for (i, edge) in map.edges.iter().enumerate() {
            assert!(
                edge.from < n && edge.to < n && edge.from != edge.to,
                "edge {i} has invalid endpoints"
            );
            assert!(
                edge.half_width.is_finite() && edge.half_width > 0.0,
                "edge {i} needs a positive finite half width"
            );
            let length = map.edge_length(i) as f64;
            assert!(
                length.is_finite() && length > 0.0,
                "edge {i} needs a positive finite length"
            );
            let radius = edge.half_width as f64;
            let resistance = 8.0 * viscosity as f64 * length / (PI * radius.powi(4));
            assert!(
                resistance.is_finite() && resistance > 0.0,
                "edge {i} has invalid resistance"
            );
            ends.push((edge.from, edge.to));
            conductances.push(1.0 / resistance);
        }

        // Node 0 is the pressure reference. For each edge, b has +1 at its
        // source and -1 at its destination: B G B^T p = -B G h.
        let size = n - 1;
        let mut matrix = vec![vec![0.0_f64; size]; size];
        let mut rhs = vec![0.0_f64; size];
        for (i, &(from, to)) in ends.iter().enumerate() {
            let g = conductances[i];
            if from > 0 {
                matrix[from - 1][from - 1] += g;
            }
            if to > 0 {
                matrix[to - 1][to - 1] += g;
            }
            if from > 0 && to > 0 {
                matrix[from - 1][to - 1] -= g;
                matrix[to - 1][from - 1] -= g;
            }
            if i == map.pump_edge {
                if from > 0 {
                    rhs[from - 1] -= g;
                }
                if to > 0 {
                    rhs[to - 1] += g;
                }
            }
        }
        let reduced = cholesky_solve(matrix, rhs);
        let mut unit_pressures = vec![0.0; n];
        unit_pressures[1..].copy_from_slice(&reduced);
        let unit_rates: Vec<f64> = ends
            .iter()
            .enumerate()
            .map(|(i, &(from, to))| {
                let head = if i == map.pump_edge { 1.0 } else { 0.0 };
                (unit_pressures[from] - unit_pressures[to] + head) * conductances[i]
            })
            .collect();
        assert!(
            unit_rates[map.pump_edge] > 0.0,
            "pump edge needs a return path through the flow graph"
        );
        let pump_edge = map.pump_edge;
        let pump_area = PI * (map.edges[pump_edge].half_width as f64).powi(2);
        Self {
            unit_pressures,
            unit_rates,
            pump_edge,
            pump_area,
        }
    }

    pub fn solve(&self, pressure_drop: f32) -> GraphFlow {
        assert!(pressure_drop.is_finite(), "pressure drop must be finite");
        let scale = pressure_drop as f64;
        let rate = self.unit_rates[self.pump_edge] * scale;
        GraphFlow {
            pressures: self
                .unit_pressures
                .iter()
                .map(|p| (p * scale) as f32)
                .collect(),
            rates: self.unit_rates.iter().map(|q| (q * scale) as f32).collect(),
            summary: Flow {
                pressure_drop,
                resistance: (1.0 / self.unit_rates[self.pump_edge]) as f32,
                rate: rate as f32,
                mean_speed: (rate / self.pump_area) as f32,
            },
        }
    }
}

fn cholesky_solve(matrix: Vec<Vec<f64>>, rhs: Vec<f64>) -> Vec<f64> {
    let n = rhs.len();
    let mut lower = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let mut value = matrix[i][j];
            for k in 0..j {
                value -= lower[i][k] * lower[j][k];
            }
            if i == j {
                assert!(
                    value.is_finite() && value > 0.0,
                    "flow graph must be connected with positive resistances"
                );
                lower[i][j] = value.sqrt();
            } else {
                lower[i][j] = value / lower[j][j];
            }
        }
    }
    let mut y = vec![0.0; n];
    for i in 0..n {
        let mut value = rhs[i];
        for k in 0..i {
            value -= lower[i][k] * y[k];
        }
        y[i] = value / lower[i][i];
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut value = y[i];
        for k in i + 1..n {
            value -= lower[k][i] * x[k];
        }
        x[i] = value / lower[i][i];
    }
    x
}

/// Approximate Poiseuille velocity in each edge, blended once per adjacent edge
/// near a junction. The graph is fixed; this is not a 2D fluid solve.
pub fn sample_velocity(position: Vector, map: &MapDefinition, flow: &GraphFlow) -> Vector {
    sample_profile(position, map, flow, None)
}

/// Planar Poiseuille profile with cross-section integral Q / depth.
/// Junction blending shares the legacy approximation; straight pipes have no extra wall fade.
pub fn sample_planar_velocity(
    position: Vector,
    map: &MapDefinition,
    flow: &GraphFlow,
    depth: f32,
) -> Vector {
    assert!(depth.is_finite() && depth > 0.0);
    sample_profile(position, map, flow, Some(depth))
}

fn sample_profile(
    position: Vector,
    map: &MapDefinition,
    flow: &GraphFlow,
    depth: Option<f32>,
) -> Vector {
    if map.edges.is_empty() || flow.rates.len() != map.edges.len() {
        return Vector::ZERO;
    }
    let (nearest, signed_distance) = map.nearest_edge_and_signed_distance(position);
    let nearest_projection = map.project_edge(nearest, position);
    let mut velocity = edge_velocity(nearest, &nearest_projection, map, flow, depth);
    let mut best_junction = None;
    for (node_id, node) in map.nodes.iter().enumerate() {
        let radius = map
            .edges
            .iter()
            .filter(|edge| edge.from == node_id || edge.to == node_id)
            .map(|edge| edge.half_width * 2.0)
            .fold(0.0_f32, f32::max);
        let distance = (position - Vector::new(node[0], node[1])).length();
        if distance < radius && best_junction.map_or(true, |(_, d, _)| distance < d) {
            best_junction = Some((node_id, distance, radius));
        }
    }
    if let Some((node_id, _, radius)) = best_junction {
        let mut total = Vector::ZERO;
        let mut weight_sum = 0.0;
        for (i, edge) in map.edges.iter().enumerate() {
            if edge.from != node_id && edge.to != node_id {
                continue;
            }
            let projection = map.project_edge(i, position);
            let lateral = (1.0 - projection.distance / edge.half_width).clamp(0.0, 1.0);
            let path_distance = if edge.from == node_id {
                projection.along
            } else {
                map.edge_length(i) - projection.along
            };
            let axial = (1.0 - path_distance / radius).clamp(0.0, 1.0);
            let weight = lateral * axial;
            if weight > 0.0 {
                total += edge_velocity(i, &projection, map, flow, depth) * weight;
                weight_sum += weight;
            }
        }
        if weight_sum > 0.0 {
            velocity = total / weight_sum;
        }
    }
    if depth.is_some() {
        return if signed_distance < 0.0 {
            velocity
        } else {
            Vector::ZERO
        };
    }
    // signed_distance is negative inside the shared union of vessel channels.
    let fade_width = map.edges[nearest].half_width * 0.25;
    let clearance = -signed_distance;
    velocity * smoothstep((clearance / fade_width).clamp(0.0, 1.0))
}

fn edge_velocity(
    edge_id: usize,
    projection: &crate::map::EdgeProjection,
    map: &MapDefinition,
    flow: &GraphFlow,
    depth: Option<f32>,
) -> Vector {
    let edge = &map.edges[edge_id];
    let radius = edge.half_width;
    let profile = (1.0 - (projection.distance / radius).powi(2)).max(0.0);
    let peak = match depth {
        Some(depth) => 3.0 * flow.rates[edge_id] / (4.0 * radius * depth),
        None => 2.0 * (flow.rates[edge_id] / (std::f32::consts::PI * radius * radius)),
    };
    projection.tangent * (peak * profile)
}

fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::VesselEdge;

    fn close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 2e-5, "{actual} != {expected}");
    }

    #[test]
    fn series_parallel_loop_obeys_pressures_conservation_and_scaling() {
        // R: 0->1 = 1; 1->2 = 2; 1->3->2 = 2+2; pump 2->0 = 1.
        // Equivalent R = 1 + (2 || 4) + 1 = 10/3. At unit head,
        // total Q = 0.3 and branch Qs = 0.2, 0.1.
        let nodes: Vec<[f32; 2]> = vec![[0.0, 0.0], [2.0, 0.0], [4.0, 0.0], [3.0, 2.0]];
        let specs = [
            (0, 1, 1.0),
            (1, 2, 2.0),
            (1, 3, 2.0),
            (3, 2, 2.0),
            (2, 0, 1.0),
        ];
        let edges = specs
            .into_iter()
            .map(|(from, to, resistance): (usize, usize, f32)| {
                let a = nodes[from];
                let b = nodes[to];
                let length = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
                let half_width = (8.0 * length / (std::f32::consts::PI * resistance)).powf(0.25);
                VesselEdge {
                    from,
                    to,
                    centerline: vec![a, b],
                    half_width,
                }
            })
            .collect();
        let map = MapDefinition {
            fluid: None,
            nodes,
            edges,
            pump_edge: 4,
            spawn: [0.0, 0.0],
            landmarks: vec![],
        };
        let network = GraphNetwork::new(&map, 1.0);
        let flow = network.solve(1.0);
        for (actual, expected) in flow.rates.iter().zip([0.3, 0.2, 0.1, 0.1, 0.3]) {
            close(*actual, expected);
        }
        for (actual, expected) in flow.pressures.iter().zip([0.0, -0.3, -0.7, -0.5]) {
            close(*actual, expected);
        }
        close(flow.summary.rate, 0.3);
        close(flow.summary.resistance, 10.0 / 3.0);
        for node in 0..map.nodes.len() {
            let balance: f32 = map
                .edges
                .iter()
                .zip(&flow.rates)
                .map(|(edge, rate)| {
                    if edge.from == node {
                        *rate
                    } else if edge.to == node {
                        -*rate
                    } else {
                        0.0
                    }
                })
                .sum();
            close(balance, 0.0);
        }
        let doubled = network.solve(2.0);
        for (actual, previous) in doubled.rates.iter().zip(&flow.rates) {
            close(*actual, 2.0 * previous);
        }
        for (actual, previous) in doubled.pressures.iter().zip(&flow.pressures) {
            close(*actual, 2.0 * previous);
        }
    }

    #[test]
    fn velocity_is_static_without_head_and_smooth_near_junction() {
        let map = MapDefinition::small_world();
        let network = GraphNetwork::new(&map, 1.0);
        let still = network.solve(0.0);
        let moving = network.solve(4.0);
        let middle = map.edge_point(0, map.edge_length(0) * 0.5, 0.0);
        assert_eq!(sample_velocity(middle, &map, &still), Vector::ZERO);
        let direct = sample_velocity(middle, &map, &moving);
        assert!(direct.x > 0.0 && direct.y.abs() < direct.x * 0.05);
        let near_wall = map.edge_point(0, map.edge_length(0) * 0.5, map.edges[0].half_width * 0.9);
        assert!(sample_velocity(near_wall, &map, &moving).length() < direct.length());
        let outside = Vector::new(0.0, -100.0);
        assert_eq!(sample_velocity(outside, &map, &moving), Vector::ZERO);
        let junction = Vector::new(map.nodes[1][0], map.nodes[1][1]);
        let a = sample_velocity(junction + Vector::new(-0.02, 0.0), &map, &moving);
        let b = sample_velocity(junction + Vector::new(0.02, 0.0), &map, &moving);
        assert!((a - b).length() < direct.length() * 0.15);

        // Subdividing a polyline does not add another vote for that edge.
        let mut subdivided = map.clone();
        for edge in &mut subdivided.edges {
            edge.centerline = edge
                .centerline
                .windows(2)
                .flat_map(|pair| {
                    [
                        pair[0],
                        [
                            (pair[0][0] + pair[1][0]) * 0.5,
                            (pair[0][1] + pair[1][1]) * 0.5,
                        ],
                    ]
                })
                .chain(edge.centerline.last().copied())
                .collect();
        }
        let refined = sample_velocity(junction, &subdivided, &moving);
        assert!((refined - sample_velocity(junction, &map, &moving)).length() < 1e-5);
    }
}
