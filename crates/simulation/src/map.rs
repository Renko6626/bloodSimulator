//! A small, fixed vessel network and the shared geometry used by physics and drawing.
mod chamber;
pub use chamber::ChamberGeometry;

pub mod fluid_asset;
pub mod fluid_policy;
mod geometry;

use fluid_policy::MapFluidDefinition;
use rapier2d::prelude::Vector;
use std::collections::BTreeSet;

pub const CHUNK_SIZE: f32 = blood_fluid::WORLD_CHUNK_SIZE as f32;
pub const SCENE_RADIUS: i32 = 2;
const GEOMETRY_SPACING: f32 = 0.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChunkId {
    pub x: i32,
    pub y: i32,
}

impl ChunkId {
    pub fn at(p: [f32; 2]) -> Self {
        Self {
            x: (p[0] / CHUNK_SIZE).floor() as i32,
            y: (p[1] / CHUNK_SIZE).floor() as i32,
        }
    }
    pub fn bounds(self) -> [[f32; 2]; 2] {
        let min = [self.x as f32 * CHUNK_SIZE, self.y as f32 * CHUNK_SIZE];
        [min, [min[0] + CHUNK_SIZE, min[1] + CHUNK_SIZE]]
    }
    pub fn neighborhood(self, radius: i32) -> BTreeSet<Self> {
        (self.y - radius..=self.y + radius)
            .flat_map(|y| (self.x - radius..=self.x + radius).map(move |x| Self { x, y }))
            .collect()
    }
    pub fn scene_bounds(self) -> [[f32; 2]; 2] {
        [
            Self {
                x: self.x - SCENE_RADIUS,
                y: self.y - SCENE_RADIUS,
            }
            .bounds()[0],
            Self {
                x: self.x + SCENE_RADIUS,
                y: self.y + SCENE_RADIUS,
            }
            .bounds()[1],
        ]
    }
}

#[derive(Clone, Debug)]
pub struct ChunkGeometry {
    pub id: ChunkId,
    pub core: MapGeometry,
    /// Includes two geometry cells of bleed for filtered background tiles.
    pub paint: MapGeometry,
}

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
    /// None preserves the legacy map flow behavior.
    pub fluid: Option<MapFluidDefinition>,
    pub nodes: Vec<[f32; 2]>,
    pub edges: Vec<VesselEdge>,
    pub chambers: Vec<ChamberGeometry>,
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
            fluid: None,
            nodes,
            edges,
            chambers: vec![],
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

    pub fn build_chunk_geometry(&self, id: ChunkId) -> ChunkGeometry {
        let [min, max] = id.bounds();
        let bleed = 2. * GEOMETRY_SPACING;
        ChunkGeometry {
            id,
            core: geometry::build_region(self, min, max, GEOMETRY_SPACING),
            paint: geometry::build_region(
                self,
                min.map(|v| v - bleed),
                max.map(|v| v + bleed),
                GEOMETRY_SPACING,
            ),
        }
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

    /// Union wall score: negative inside a vessel or chamber, positive outside.
    pub fn signed_distance(&self, position: Vector) -> f32 {
        self.edges
            .iter()
            .map(|edge| {
                distance_to_path_squared(&edge.centerline, position).sqrt() - edge.half_width
            })
            .chain(
                self.chambers
                    .iter()
                    .map(|chamber| chamber.signed_distance(position)),
            )
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
        for chamber in &self.chambers {
            signed_distance = signed_distance.min(chamber.signed_distance(position));
        }
        (nearest, signed_distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concave_chamber_has_signed_walls_and_closes_its_last_edge() {
        let outline = vec![[0., 0.], [6., 0.], [6., 2.], [2., 2.], [2., 6.], [0., 6.]];
        for boundary in [outline.clone(), outline.into_iter().rev().collect()] {
            let chamber = ChamberGeometry::new("concave", boundary);
            assert_eq!(chamber.bounds(), [[0., 0.], [6., 6.]]);
            assert_eq!(chamber.signed_distance(v([1., 4.])), -1.);
            assert_eq!(chamber.signed_distance(v([4., 4.])), 2.);
            assert_eq!(chamber.signed_distance(v([0., 4.])), 0.);
            assert_eq!(chamber.signed_distance(v([-1., 4.])), 1.);
        }
    }

    #[test]
    fn chamber_union_preserves_graph_ownership_and_shared_wall_score() {
        let old = MapDefinition::small_world();
        let mut map = old.clone();
        map.chambers.push(ChamberGeometry::new(
            "extension",
            vec![[-22., -8.], [-14., -8.], [-14., 2.], [-22., 2.]],
        ));
        // Chamber-only interior, tube overlap, a former chamber edge inside the tube,
        // a tube-only point, and tissue outside both.
        for point in [
            [-18., -6.],
            [-18., 0.],
            [-14., 0.],
            [9., -12.],
            [-18., -10.],
        ] {
            let point = v(point);
            let (edge, score) = map.nearest_edge_and_signed_distance(point);
            assert_eq!(edge, old.nearest_edge(point));
            assert_eq!(score, map.signed_distance(point));
        }
        assert_eq!(map.signed_distance(v([-18., -6.])), -2.);
        assert!(map.signed_distance(v([-14., 0.])) < -2.);
        assert!(map.signed_distance(v([-18., -10.])) > 0.);
    }

    #[test]
    fn chamber_outside_tubes_generates_floor_and_unsealed_chunk_walls() {
        let mut map = MapDefinition::small_world();
        map.chambers.push(ChamberGeometry::new(
            "distant",
            vec![[72., 68.], [104., 68.], [104., 76.], [72., 76.]],
        ));
        let whole = map.build_geometry(0.2);
        assert!(whole.bounds[1][0] > 104. && whole.bounds[1][1] > 76.);
        assert!(
            whole
                .floor_vertices
                .iter()
                .any(|p| p[0] > 90. && p[1] > 70.)
        );
        assert!(whole.walls.iter().flatten().any(|p| p[0] > 90.));
        let a = map.build_chunk_geometry(ChunkId { x: 4, y: 4 });
        let b = map.build_chunk_geometry(ChunkId { x: 5, y: 4 });
        assert!(!a.core.floor_vertices.is_empty() && !b.core.floor_vertices.is_empty());
        let walls: Vec<_> = a.core.walls.iter().chain(&b.core.walls).collect();
        assert!(walls.iter().all(|segment| segment.windows(2).all(|ends| {
            let midpoint = (v(ends[0]) + v(ends[1])) * 0.5;
            map.signed_distance(midpoint).abs() < 0.15
        })));
        // The chamber crosses x=80 through open floor, with no artificial seam wall.
        assert!(walls.iter().all(|segment| segment.iter().all(|p| {
            (p[0] - 80.).abs() > 1e-5 || (p[1] - 68.).abs() < 1e-5 || (p[1] - 76.).abs() < 1e-5
        })));
        assert!(
            walls
                .iter()
                .any(|segment| segment.iter().any(|p| p[0] == 80.))
        );
    }

    #[test]
    fn fixed_chunks_share_geometry_without_sealing_the_channel() {
        let map = MapDefinition {
            fluid: None,
            chambers: vec![],
            nodes: vec![[-40., 0.], [40., 0.]],
            edges: vec![VesselEdge {
                from: 0,
                to: 1,
                centerline: vec![[-40., 0.], [40., 0.]],
                half_width: 3.,
            }],
            pump_edge: 0,
            spawn: [0., 0.],
            landmarks: vec![],
        };
        assert_eq!(ChunkId::at([-0.01, 0.]), ChunkId { x: -1, y: 0 });
        let a = map.build_chunk_geometry(ChunkId { x: -1, y: 0 });
        let b = map.build_chunk_geometry(ChunkId { x: 0, y: 0 });
        assert_eq!(a.core.bounds[1][0], b.core.bounds[0][0]);
        let mut walls = a.core.walls.clone();
        walls.extend(b.core.walls.clone());
        assert!(
            walls
                .iter()
                .all(|w| w.iter().all(|p| (p[1] - 3.).abs() < 1e-5))
        );
        assert!(walls.iter().any(|w| w.iter().any(|p| p[0] == 0.)));
        let length: f32 = walls.iter().map(|w| (v(w[1]) - v(w[0])).length()).sum();
        assert!((length - 32.).abs() < 1e-4, "wall length {length}");
        assert_eq!(ChunkId::at([5., 5.]).neighborhood(2).len(), 25);
    }

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
