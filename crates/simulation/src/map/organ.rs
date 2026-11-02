//! Candidate A: hand-authored organ streets, separate from the graded heart fixture.
use super::{Landmark, MapDefinition, VesselEdge};
type Point = [f32; 2];

struct Road {
    name: &'static str,
    diameter: f32,
    points: Vec<Point>,
}

// Source: the approved candidate A drawing. Names identify logical roads; actual
// graph edges split at shared junctions and at the four narrow port transitions.
fn roads() -> Vec<Road> {
    vec![
        Road {
            name: "肺出接近段",
            diameter: 24.,
            points: vec![[528., 646.], [528., 550.]],
        },
        Road {
            name: "肺供西廊",
            diameter: 12.,
            points: vec![
                [528., 550.],
                [480., 500.],
                [440., 400.],
                [450., 220.],
                [490., 140.],
            ],
        },
        Road {
            name: "肺西外环",
            diameter: 12.,
            points: vec![[450., 220.], [400., 260.], [400., 360.], [440., 400.]],
        },
        Road {
            name: "肺回接近段",
            diameter: 24.,
            points: vec![[562., 646.], [562., 550.]],
        },
        Road {
            name: "肺回东廊",
            diameter: 12.,
            points: vec![
                [562., 550.],
                [620., 500.],
                [660., 400.],
                [650., 220.],
                [610., 140.],
            ],
        },
        Road {
            name: "肺东外环",
            diameter: 12.,
            points: vec![[650., 220.], [700., 270.], [700., 360.], [660., 400.]],
        },
        Road {
            name: "肺上台地北路",
            diameter: 6.,
            points: vec![
                [490., 140.],
                [520., 100.],
                [565., 115.],
                [585., 90.],
                [610., 140.],
            ],
        },
        Road {
            name: "肺上台地南路",
            diameter: 6.,
            points: vec![[490., 140.], [520., 180.], [570., 170.], [610., 140.]],
        },
        Road {
            name: "肺中部曲路",
            diameter: 6.,
            points: vec![
                [440., 400.],
                [480., 350.],
                [530., 370.],
                [560., 320.],
                [615., 350.],
                [660., 400.],
            ],
        },
        Road {
            name: "肺中部南路",
            diameter: 6.,
            points: vec![
                [440., 400.],
                [490., 420.],
                [540., 400.],
                [590., 430.],
                [660., 400.],
            ],
        },
        Road {
            name: "肺下口北路",
            diameter: 6.,
            points: vec![
                [480., 500.],
                [490., 460.],
                [530., 450.],
                [580., 470.],
                [620., 500.],
            ],
        },
        Road {
            name: "肺下口南路",
            diameter: 6.,
            points: vec![
                [480., 500.],
                [500., 490.],
                [540., 480.],
                [580., 490.],
                [620., 500.],
            ],
        },
        Road {
            name: "肺中央纵街",
            diameter: 6.,
            points: vec![[570., 170.], [590., 230.], [555., 260.], [560., 320.]],
        },
        Road {
            name: "中央汇口西入口",
            diameter: 6.,
            points: vec![[450., 220.], [490., 240.], [520., 250.], [555., 260.]],
        },
        Road {
            name: "中央汇口东入口",
            diameter: 6.,
            points: vec![[650., 220.], [620., 240.], [555., 260.]],
        },
        Road {
            name: "上台地窄联络",
            diameter: 3.,
            points: vec![[565., 115.], [555., 140.], [570., 170.]],
        },
        Road {
            name: "体供外围大道",
            diameter: 24.,
            points: vec![
                [600., 646.],
                [700., 600.],
                [850., 650.],
                [940., 820.],
                [880., 980.],
                [600., 1030.],
                [320., 970.],
                [180., 800.],
                [210., 600.],
                [300., 450.],
            ],
        },
        Road {
            name: "东侧供血支环",
            diameter: 12.,
            points: vec![[850., 650.], [1000., 680.], [1030., 810.], [940., 820.]],
        },
        Road {
            name: "南侧供血支环",
            diameter: 12.,
            points: vec![[600., 1030.], [470., 1100.], [310., 1060.], [320., 970.]],
        },
        Road {
            name: "西北供血支环",
            diameter: 12.,
            points: vec![[210., 600.], [130., 540.], [180., 420.], [300., 450.]],
        },
        Road {
            name: "体回内侧大道",
            diameter: 24.,
            points: vec![
                [488., 646.],
                [400., 600.],
                [340., 700.],
                [380., 850.],
                [600., 910.],
                [780., 850.],
                [820., 700.],
            ],
        },
        Road {
            name: "南内回流支环",
            diameter: 12.,
            points: vec![
                [380., 850.],
                [400., 910.],
                [450., 940.],
                [500., 950.],
                [560., 940.],
                [600., 910.],
            ],
        },
        Road {
            name: "东内回流支环",
            diameter: 12.,
            points: vec![[780., 850.], [720., 760.], [740., 670.], [820., 700.]],
        },
        Road {
            name: "A上折路",
            diameter: 6.,
            points: vec![
                [940., 820.],
                [900., 800.],
                [860., 760.],
                [850., 720.],
                [820., 700.],
            ],
        },
        Road {
            name: "A下折路",
            diameter: 6.,
            points: vec![
                [940., 820.],
                [900., 870.],
                [850., 850.],
                [820., 870.],
                [780., 850.],
            ],
        },
        Road {
            name: "A南接近",
            diameter: 6.,
            points: vec![
                [880., 980.],
                [850., 930.],
                [800., 920.],
                [760., 900.],
                [780., 850.],
            ],
        },
        Road {
            name: "B西弯路",
            diameter: 6.,
            points: vec![[600., 1030.], [550., 1000.], [565., 970.], [600., 910.]],
        },
        Road {
            name: "B东弯路",
            diameter: 6.,
            points: vec![[600., 1030.], [640., 1000.], [650., 950.], [600., 910.]],
        },
        Road {
            name: "B窄联络",
            diameter: 3.,
            points: vec![[565., 970.], [600., 985.], [650., 950.]],
        },
        Road {
            name: "C北弯路",
            diameter: 6.,
            points: vec![
                [300., 450.],
                [320., 500.],
                [320., 550.],
                [360., 570.],
                [400., 600.],
            ],
        },
        Road {
            name: "C南弯路",
            diameter: 6.,
            points: vec![[210., 600.], [230., 650.], [280., 670.], [340., 700.]],
        },
        Road {
            name: "C中街",
            diameter: 6.,
            points: vec![[320., 550.], [330., 600.], [280., 670.]],
        },
    ]
}

fn node(nodes: &mut Vec<Point>, point: Point) -> usize {
    if let Some(index) = nodes.iter().position(|p| *p == point) {
        index
    } else {
        nodes.push(point);
        nodes.len() - 1
    }
}

fn add_path(nodes: &mut Vec<Point>, edges: &mut Vec<VesselEdge>, path: &[Point], half_width: f32) {
    edges.push(VesselEdge {
        from: node(nodes, path[0]),
        to: node(nodes, *path.last().unwrap()),
        centerline: path.to_vec(),
        half_width,
    });
}

impl MapDefinition {
    pub fn organ_world() -> Self {
        let translate = |p: Point| [p[0] + 450., p[1] + 650.];
        let mut chambers = super::heart::chambers();
        for chamber in &mut chambers {
            for p in &mut chamber.boundary {
                *p = translate(*p);
            }
        }
        // These are the unchanged eight intracardiac guides, translated 1:1.
        let paths: &[(&[Point], f32)] = &[
            (&[[38., -4.], [38., 32.], [45., 45.]], 5.),
            (&[[45., 45.], [50., 62.], [49., 78.], [49., 86.]], 5.),
            (
                &[
                    [49., 86.],
                    [49., 110.],
                    [61., 170.],
                    [62., 100.],
                    [69., 85.],
                ],
                5.,
            ),
            (&[[69., 85.], [78., 65.], [78., 55.], [78., -4.]], 4.5),
            (&[[112., -4.], [112., 45.]], 5.),
            (&[[112., 45.], [108., 63.], [106., 78.], [106., 86.]], 5.),
            (&[[106., 86.], [106., 176.], [123., 106.], [132., 86.]], 5.),
            (&[[132., 86.], [150., 65.], [150., -4.]], 5.),
        ];
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for (path, radius) in paths {
            let path: Vec<_> = path.iter().copied().map(translate).collect();
            add_path(&mut nodes, &mut edges, &path, *radius);
        }
        let roads = roads();
        for road in &roads {
            let mut first = 0;
            for last in 1..road.points.len() {
                let p = road.points[last];
                let junction = roads.iter().filter(|r| r.points.contains(&p)).count() > 1;
                if last + 1 == road.points.len() || junction {
                    let path = &road.points[first..=last];
                    if first == 0 && road.diameter == 24. {
                        // Keep the wide round cap away from the original chamber
                        // mouths. A short narrow neck joins the fixed-width tube.
                        let port = path[0];
                        let delta = super::v(path[1]) - super::v(port);
                        let neck = (super::v(port) + delta.normalize() * 20.).to_array();
                        let radius = if port == [528., 646.] { 4.5 } else { 5. };
                        add_path(&mut nodes, &mut edges, &[port, neck], radius);
                        let mut wide = vec![neck];
                        wide.extend_from_slice(&path[1..]);
                        add_path(&mut nodes, &mut edges, &wide, 12.);
                    } else {
                        add_path(&mut nodes, &mut edges, path, road.diameter * 0.5);
                    }
                    first = last;
                }
            }
        }
        let mut landmarks: Vec<_> = roads
            .iter()
            .map(|road| Landmark {
                name: road.name.into(),
                position: road.points[road.points.len() / 2],
                size: [4., 2.],
            })
            .collect();
        landmarks.extend(
            [
                ("心脏", [550., 750.], [100., 150.]),
                ("肺部", [550., 300.], [355., 475.]),
                ("器官预留区 A", [900., 820.], [210., 310.]),
                ("器官预留区 B", [590., 985.], [220., 120.]),
                ("器官预留区 C", [300., 625.], [205., 170.]),
            ]
            .map(|(name, position, size)| Landmark {
                name: name.into(),
                position,
                size,
            }),
        );
        Self {
            fluid: None,
            nodes,
            edges,
            chambers,
            pump_edge: 7,
            spawn: translate([49., 72.]),
            landmarks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, circulation::graph::GraphNetwork};
    use std::collections::BTreeSet;

    fn reachable(map: &MapDefinition, start: usize, first_edge: usize) -> BTreeSet<usize> {
        let mut seen = BTreeSet::from([start]);
        let mut pending = vec![start];
        while let Some(n) = pending.pop() {
            for e in &map.edges[first_edge..] {
                let next = if e.from == n {
                    Some(e.to)
                } else if e.to == n {
                    Some(e.from)
                } else {
                    None
                };
                if let Some(next) = next {
                    if seen.insert(next) {
                        pending.push(next);
                    }
                }
            }
        }
        seen
    }

    #[test]
    fn organ_junctions_connect_one_cycle_of_two_separate_external_networks() {
        let map = MapDefinition::organ_world();
        assert_eq!(reachable(&map, 0, 0).len(), map.nodes.len());
        let at = |p| map.nodes.iter().position(|q| *q == p).unwrap();
        let lung = reachable(&map, at([528., 646.]), 8);
        let body = reachable(&map, at([600., 646.]), 8);
        assert!(lung.contains(&at([562., 646.])));
        assert!(body.contains(&at([488., 646.])));
        assert!(lung.is_disjoint(&body));
        // Every shared drawing vertex is a real graph junction, even when it
        // appears in the middle of a logical road (central street and C street).
        let roads = roads();
        for road in &roads {
            for &p in &road.points {
                if roads.iter().filter(|r| r.points.contains(&p)).count() > 1 {
                    let n = at(p);
                    let mut neighbors = Vec::new();
                    for r in &roads {
                        for pair in r.points.windows(2) {
                            if pair.contains(&p) {
                                let other = if pair[0] == p { pair[1] } else { pair[0] };
                                if !neighbors.contains(&other) {
                                    neighbors.push(other);
                                }
                            }
                        }
                    }
                    assert_eq!(
                        map.edges
                            .iter()
                            .filter(|e| e.from == n || e.to == n)
                            .count(),
                        neighbors.len(),
                        "junction {p:?}"
                    );
                }
            }
        }
        assert_eq!(map.edges.iter().filter(|e| e.half_width == 1.5).count(), 2);
        // E-V+C counts external independent cycles without counting bend points.
        let external_nodes: BTreeSet<_> =
            map.edges[8..].iter().flat_map(|e| [e.from, e.to]).collect();
        assert_eq!(map.edges.len() - 8 + 2 - external_nodes.len(), 24);
    }

    #[test]
    fn organ_preserves_chambers_and_centerline_clearance_without_extra_mouths() {
        let map = MapDefinition::organ_world();
        let original = super::super::heart::chambers();
        for (a, b) in map.chambers.iter().zip(&original) {
            assert_eq!(a.name, b.name);
            for (p, q) in a.boundary.iter().zip(&b.boundary) {
                assert_eq!(*p, [q[0] + 450., q[1] + 650.]);
            }
        }
        for (index, edge) in map.edges.iter().enumerate() {
            assert_eq!(edge.centerline.first(), Some(&map.nodes[edge.from]));
            assert_eq!(edge.centerline.last(), Some(&map.nodes[edge.to]));
            for pair in edge.centerline.windows(2) {
                let length = (super::super::v(pair[1]) - super::super::v(pair[0])).length();
                for i in 0..=(length.ceil() as usize) {
                    let t = i as f32 / length.ceil();
                    let p = super::super::v(pair[0]) * (1. - t) + super::super::v(pair[1]) * t;
                    assert!(map.signed_distance(p) <= -1.49, "edge {index} point {p:?}");
                    if index >= 8 {
                        for chamber in &map.chambers {
                            assert!(
                                chamber.signed_distance(p) > edge.half_width + 1.,
                                "edge {index} touches {} at {p:?}",
                                chamber.name
                            );
                        }
                    }
                }
            }
        }
        // Check the actual capsule widths, including round caps and bends.
        // Incident segments intentionally merge at their shared junction.
        let segments: Vec<_> = map.edges[8..]
            .iter()
            .flat_map(|e| {
                e.centerline
                    .windows(2)
                    .map(move |pair| (pair, e.half_width))
            })
            .collect();
        let mut minimum_gap = f32::INFINITY;
        for (index, &(a, radius_a)) in segments.iter().enumerate() {
            for &(b, radius_b) in &segments[index + 1..] {
                if a.iter().any(|p| b.contains(p)) {
                    continue;
                }
                let length = (super::super::v(a[1]) - super::super::v(a[0])).length();
                for i in 0..=length.ceil() as usize {
                    let t = i as f32 / length.ceil();
                    let p = super::super::v(a[0]) * (1. - t) + super::super::v(a[1]) * t;
                    let gap =
                        super::super::distance_to_path_squared(b, p).sqrt() - radius_a - radius_b;
                    minimum_gap = minimum_gap.min(gap);
                    assert!(gap > 1.5, "unintended capsule contact {a:?} {b:?}: {gap}");
                }
            }
        }
        eprintln!("organ sampled nonincident external wall gap >= {minimum_gap}");
        // The preserved septum and the reserved region fields stay solid.
        for p in [[531., 775.], [930., 740.], [600., 970.], [300., 625.]] {
            assert!(
                map.signed_distance(super::super::v(p)) > 0.,
                "unexpected floor {p:?}"
            );
        }
    }

    #[test]
    fn organ_unit_drive_is_finite_and_conservative_without_branch_equalization() {
        let map = MapDefinition::organ_world();
        let flow = GraphNetwork::new(&map, Config::default().viscosity).solve(1.);
        assert!(
            flow.pressures
                .iter()
                .chain(&flow.rates)
                .all(|v| v.is_finite())
        );
        assert!(flow.summary.rate > 0.);
        let mut maximum_residual = 0_f32;
        for n in 0..map.nodes.len() {
            let residual: f32 = map
                .edges
                .iter()
                .zip(&flow.rates)
                .map(|(e, q)| {
                    if e.from == n {
                        *q
                    } else if e.to == n {
                        -*q
                    } else {
                        0.
                    }
                })
                .sum();
            maximum_residual = maximum_residual.max(residual.abs());
            assert!(
                residual.abs() < flow.summary.rate * 1e-5,
                "node {n}: {residual}"
            );
        }
        eprintln!(
            "organ unit drive: nodes={} edges={} Q={} viscosity={} max_residual={maximum_residual}",
            map.nodes.len(),
            map.edges.len(),
            flow.summary.rate,
            Config::default().viscosity
        );
        let roads = roads();
        for (i, e) in map.edges.iter().enumerate() {
            let name = roads
                .iter()
                .find(|r| {
                    e.centerline.iter().all(|p| {
                        super::super::distance_to_path_squared(&r.points, super::super::v(*p))
                            < 1e-6
                    })
                })
                .map(|r| r.name)
                .unwrap_or("心内导引");
            eprintln!(
                "edge={i} road={name} from={:?} to={:?} D={} Q={:.9} ratio={:.9}",
                map.nodes[e.from],
                map.nodes[e.to],
                e.half_width * 2.,
                flow.rates[i],
                flow.rates[i] / flow.summary.rate
            );
        }
    }
}
