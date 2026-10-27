//! A small, fixed vessel network and the shared geometry used by physics and drawing.
mod geometry;

use rapier2d::prelude::Vector;

#[derive(Clone, Debug)]
pub struct VesselEdge {
    pub from: usize,
    pub to: usize,
    pub centerline: Vec<[f32; 2]>,
    pub half_width: f32,
}

#[derive(Clone, Debug)]
pub struct Landmark {
    pub name: String,
    pub position: [f32; 2],
    pub size: [f32; 2],
}

#[derive(Clone, Debug)]
pub struct MapDefinition {
    pub nodes: Vec<[f32; 2]>,
    pub edges: Vec<VesselEdge>,
    pub pump_edge: usize,
    pub spawn: [f32; 2],
    pub landmarks: Vec<Landmark>,
}

#[derive(Clone, Debug)]
pub struct EdgeProjection {
    pub point: Vector,
    pub tangent: Vector,
    pub distance: f32,
    pub along: f32,
}

#[derive(Clone, Debug)]
pub struct MapGeometry {
    pub walls: Vec<Vec<[f32; 2]>>,
    pub floor_vertices: Vec<[f32; 2]>,
    pub bounds: [[f32; 2]; 2],
}

fn v(point: [f32; 2]) -> Vector {
    Vector::new(point[0], point[1])
}

fn distance_to_path_squared(path: &[[f32; 2]], position: Vector) -> f32 {
    let mut best = f32::INFINITY;
    for pair in path.windows(2) {
        let start = v(pair[0]);
        let delta = v(pair[1]) - start;
        let length_squared = delta.length_squared();
        if length_squared <= f32::EPSILON * f32::EPSILON {
            continue;
        }
        let t = ((position - start).dot(delta) / length_squared).clamp(0.0, 1.0);
        best = best.min((position - (start + delta * t)).length_squared());
    }
    best
}

fn bezier(start: [f32; 2], control: [f32; 2], end: [f32; 2]) -> Vec<[f32; 2]> {
    let a = v(start);
    let b = v(control);
    let c = v(end);
    (0..=16)
        .map(|i| {
            let t = i as f32 / 16.0;
            (a * (1.0 - t).powi(2) + b * (2.0 * t * (1.0 - t)) + c * t).to_array()
        })
        .collect()
}

impl MapDefinition {
    pub fn small_world() -> Self {
        let nodes = vec![
            [-24.0, 0.0], // heart
            [-12.0, 0.0], // fork
            [0.0, -12.0],
            [18.0, -12.0],
            [0.0, 12.0],
            [18.0, 12.0],
            [30.0, 0.0], // join
            [42.0, 10.0],
            [36.0, 26.0],
            [-16.0, 26.0],
            [-28.0, 12.0],
        ];
        let mut edges = Vec::new();
        let mut add = |from, to, control: Option<[f32; 2]>, half_width| {
            let centerline = match control {
                Some(control) => bezier(nodes[from], control, nodes[to]),
                None => vec![nodes[from], nodes[to]],
            };
            edges.push(VesselEdge {
                from,
                to,
                centerline,
                half_width,
            });
        };
        add(0, 1, None, 3.5);
        add(1, 2, Some([-7.0, -11.0]), 3.1);
        add(2, 3, None, 3.1);
        add(3, 6, Some([26.0, -10.0]), 3.1);
        add(1, 4, Some([-7.0, 11.0]), 3.1);
        add(4, 5, None, 3.1);
        add(5, 6, Some([26.0, 10.0]), 3.1);
        add(6, 7, Some([41.0, 0.0]), 3.5);
        add(7, 8, Some([46.0, 23.0]), 3.5);
        add(8, 9, None, 3.5);
        add(9, 10, Some([-27.0, 25.0]), 3.5);
        add(10, 0, Some([-28.0, 3.0]), 3.5);
        Self {
            nodes,
            edges,
            pump_edge: 11,
            spawn: [-20.0, 0.0],
            landmarks: vec![
                Landmark {
                    name: "心脏".into(),
                    position: [-24.0, 0.0],
                    size: [8.0, 8.0],
                },
                Landmark {
                    name: "器官 A".into(),
                    position: [9.0, -12.0],
                    size: [15.0, 7.0],
                },
                Landmark {
                    name: "器官 B".into(),
                    position: [9.0, 12.0],
                    size: [15.0, 7.0],
                },
            ],
        }
    }

    pub fn build_geometry(&self, spacing: f32) -> MapGeometry {
        geometry::build(self, spacing)
    }

    pub fn edge_length(&self, edge: usize) -> f32 {
        self.edges[edge]
            .centerline
            .windows(2)
            .map(|pair| (v(pair[1]) - v(pair[0])).length())
            .sum()
    }

    /// Positive offset lies to the left of the edge's directed tangent.
    pub fn edge_point(&self, edge: usize, along: f32, offset: f32) -> Vector {
        let path = &self.edges[edge].centerline;
        let mut remaining = along.clamp(0.0, self.edge_length(edge));
        for pair in path.windows(2) {
            let delta = v(pair[1]) - v(pair[0]);
            let length = delta.length();
            if length <= f32::EPSILON {
                continue;
            }
            if remaining <= length {
                let tangent = delta / length;
                return v(pair[0])
                    + tangent * remaining
                    + Vector::new(-tangent.y, tangent.x) * offset;
            }
            remaining -= length;
        }
        let last = path.len() - 1;
        let delta = v(path[last]) - v(path[last - 1]);
        let tangent = delta.normalize();
        v(path[last]) + Vector::new(-tangent.y, tangent.x) * offset
    }

    pub fn project_edge(&self, edge: usize, position: Vector) -> EdgeProjection {
        let mut best = EdgeProjection {
            point: v(self.edges[edge].centerline[0]),
            tangent: Vector::X,
            distance: f32::INFINITY,
            along: 0.0,
        };
        let mut walked = 0.0;
        let mut best_distance_squared = f32::INFINITY;
        for pair in self.edges[edge].centerline.windows(2) {
            let start = v(pair[0]);
            let delta = v(pair[1]) - start;
            let length_squared = delta.length_squared();
            if length_squared <= f32::EPSILON * f32::EPSILON {
                continue;
            }
            let length = length_squared.sqrt();
            let tangent = delta / length;
            let local = (position - start).dot(tangent).clamp(0.0, length);
            let point = start + tangent * local;
            let distance_squared = (position - point).length_squared();
            if distance_squared < best_distance_squared {
                best_distance_squared = distance_squared;
                best = EdgeProjection {
                    point,
                    tangent,
                    distance: f32::INFINITY,
                    along: walked + local,
                };
            }
            walked += length;
        }
        best.distance = best_distance_squared.sqrt();
        best
    }

    /// Distance to the union boundary: negative inside a vessel, positive outside.
    pub fn signed_distance(&self, position: Vector) -> f32 {
        self.edges
            .iter()
            .map(|edge| {
                distance_to_path_squared(&edge.centerline, position).sqrt() - edge.half_width
            })
            .fold(f32::INFINITY, f32::min)
    }

    pub fn nearest_edge(&self, position: Vector) -> usize {
        self.nearest_edge_and_signed_distance(position).0
    }

    /// Reuse each path distance when a caller needs both the vessel and wall score.
    pub(crate) fn nearest_edge_and_signed_distance(&self, position: Vector) -> (usize, f32) {
        let mut signed_distance = f32::INFINITY;
        let nearest = self
            .edges
            .iter()
            .enumerate()
            .map(|(index, edge)| {
                let distance =
                    distance_to_path_squared(&edge.centerline, position).sqrt() - edge.half_width;
                signed_distance = signed_distance.min(distance);
                (index, distance)
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| index)
            .expect("map has at least one edge");
        (nearest, signed_distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_has_open_shared_floor_and_boundary() {
        let map = MapDefinition::small_world();
        let geometry = map.build_geometry(0.2);
        assert!(map.nodes.len() < 16);
        for node in [1, 6] {
            let point = v(map.nodes[node]);
            assert!(map.signed_distance(point) < -2.0);
            assert!(geometry.floor_vertices.chunks_exact(3).any(|triangle| {
                let a = v(triangle[0]);
                let b = v(triangle[1]);
                let c = v(triangle[2]);
                let cross = |u: Vector, w: Vector| u.x * w.y - u.y * w.x;
                let d0 = cross(b - a, point - a);
                let d1 = cross(c - b, point - b);
                let d2 = cross(a - c, point - c);
                (d0 >= -1e-4 && d1 >= -1e-4 && d2 >= -1e-4)
                    || (d0 <= 1e-4 && d1 <= 1e-4 && d2 <= 1e-4)
            }));
            assert!(geometry.walls.iter().all(|segment| {
                segment.windows(2).all(|ends| {
                    let a = v(ends[0]);
                    let b = v(ends[1]);
                    let delta = b - a;
                    let t = ((point - a).dot(delta) / delta.length_squared()).clamp(0.0, 1.0);
                    (point - (a + delta * t)).length() > 2.0
                })
            }));
        }
        for segment in &geometry.walls {
            for point in segment {
                assert!(map.signed_distance(v(*point)).abs() < 0.15);
            }
        }
    }
}
