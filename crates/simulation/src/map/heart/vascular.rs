//! Authored heart-only external fan trees. Each four-way fan has equal actual
//! polyline length: a short isolated hairpin compensates the inner diagonals.
use super::VesselEdge;

type Point = [f32; 2];
const DEPTH: f32 = 1240.;

fn position(p: Point, pulmonary: bool) -> Point {
    let x = p[0] - DEPTH / 2.;
    [
        if pulmonary { x } else { -x },
        p[1] + if pulmonary { -800. } else { 900. },
    ]
}

fn branch(parent: Point, child: usize, level: usize) -> Vec<Point> {
    // (child spacing, forward fan, straight tail, final gap, lane gap, minimum backtrack).
    let (spacing, run, tail, gap, lane, base): (f32, f32, f32, f32, f32, f32) = match level {
        0 => (256., 128., 200., 40., 18., 20.),
        1 => (64., 64., 80., 24., 10., 12.),
        2 => (16., 32., 24., 12., 4., 8.),
        _ => unreachable!("the heart layout has exactly three fan levels"),
    };
    let offset = (child as f32 - 1.5) * spacing;
    let longest = run.hypot(1.5 * spacing);
    let back = base + (longest - run.hypot(offset)) / 2.;
    let [u, v] = parent;
    let v = v + offset;
    vec![
        parent,
        [u + run, v],
        [u + run + tail, v],
        [u + run + tail, v + lane],
        [u + run + tail - back, v + lane],
        [u + run + tail - back, v + 2. * lane],
        [u + run + tail + gap, v + 2. * lane],
        [u + run + tail + gap, v],
    ]
}

fn reflected(path: &[Point]) -> Vec<Point> {
    path.iter().rev().map(|p| [DEPTH - p[0], p[1]]).collect()
}

fn trunks(pulmonary: bool) -> (Vec<Point>, Vec<Point>) {
    if pulmonary {
        (
            vec![[78., -64.], [78., -96.], [-620., -96.], [-620., -800.]],
            vec![[620., -800.], [620., -128.], [112., -128.], [112., -64.]],
        )
    } else {
        (
            vec![[150., -64.], [700., -64.], [700., 900.], [620., 900.]],
            vec![[-620., 900.], [-700., 900.], [-700., -64.], [38., -64.]],
        )
    }
}

fn node(nodes: &mut Vec<Point>, p: Point) -> usize {
    let i = nodes.len();
    nodes.push(p);
    i
}
fn edge(
    edges: &mut Vec<VesselEdge>,
    from: usize,
    to: usize,
    centerline: Vec<Point>,
    half_width: f32,
) {
    edges.push(VesselEdge {
        from,
        to,
        centerline,
        half_width,
    });
}

fn module(
    nodes: &mut Vec<Point>,
    edges: &mut Vec<VesselEdge>,
    inlet: usize,
    outlet: usize,
    pulmonary: bool,
) {
    let (supply, venous) = trunks(pulmonary);
    let root = node(nodes, position([0., 0.], pulmonary));
    let return_root = node(nodes, position([DEPTH, 0.], pulmonary));
    edge(edges, inlet, root, supply, 12.);
    let mut parents = vec![(root, return_root, [0., 0.])];
    for level in 0..2 {
        let mut children = Vec::new();
        for (from, to, p) in parents {
            for child in 0..4 {
                let path = branch(p, child, level);
                let end = *path.last().unwrap();
                let a = node(nodes, position(end, pulmonary));
                let b = node(nodes, position([DEPTH - end[0], end[1]], pulmonary));
                let reverse = reflected(&path);
                edge(
                    edges,
                    from,
                    a,
                    path.into_iter().map(|p| position(p, pulmonary)).collect(),
                    if level == 0 { 6. } else { 3. },
                );
                edge(
                    edges,
                    b,
                    to,
                    reverse
                        .into_iter()
                        .map(|p| position(p, pulmonary))
                        .collect(),
                    if level == 0 { 6. } else { 3. },
                );
                children.push((a, b, end));
            }
        }
        parents = children;
    }
    for (from, to, p) in parents {
        for child in 0..4 {
            let mut path = branch(p, child, 2);
            let return_path = reflected(&path);
            path.extend(return_path);
            edge(
                edges,
                from,
                to,
                path.into_iter().map(|p| position(p, pulmonary)).collect(),
                1.5,
            );
        }
    }
    edge(edges, return_root, outlet, venous, 12.);
}

pub(super) fn attach(nodes: &mut Vec<Point>, edges: &mut Vec<VesselEdge>) {
    let pa = node(nodes, [78., -64.]);
    let pv = node(nodes, [112., -64.]);
    let ao = node(nodes, [150., -64.]);
    let sv = node(nodes, [38., -64.]);
    edges[4].to = pa;
    edges[9].to = ao;
    edge(edges, pv, 5, vec![nodes[pv], nodes[5]], 5.);
    edge(edges, sv, 0, vec![nodes[sv], nodes[0]], 5.);
    module(nodes, edges, pa, pv, true);
    module(nodes, edges, ao, sv, false);
}

pub(super) fn route(pulmonary: bool) -> Vec<(&'static str, Point)> {
    let (mut path, back) = trunks(pulmonary);
    let mut p = [0., 0.];
    let mut outward = Vec::new();
    for level in 0..3 {
        let segment = branch(p, 0, level);
        p = *segment.last().unwrap();
        outward.extend(segment.into_iter().skip(usize::from(level > 0)));
    }
    let inward = reflected(&outward);
    outward.extend(inward);
    path.extend(outward.into_iter().map(|p| position(p, pulmonary)));
    path.extend(back);
    path.push(if pulmonary { [112., -4.] } else { [38., -4.] });
    let mut result = Vec::new();
    for pair in path.windows(2) {
        let distance = (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
        let steps = (distance / 96.).ceil().max(1.) as usize;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            result.push((
                "vessel_waypoint",
                std::array::from_fn(|j| pair[0][j] + t * (pair[1][j] - pair[0][j])),
            ));
        }
    }
    // Include the narrow port before the first wide-trunk corner.
    result.insert(0, ("vessel_interface", path[0]));
    if pulmonary {
        result.insert(0, ("pulmonary_interface", [78., -32.]));
        result.last_mut().unwrap().0 = "lung_return";
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{circulation::graph::GraphNetwork, map::MapDefinition};
    use rapier2d::prelude::Vector;

    #[test]
    fn heart_preview_graded_graph_reference() {
        let map = MapDefinition::heart_preview();
        assert_eq!(map.nodes.len(), 98);
        assert_eq!(map.edges.len(), 224);
        let graph = GraphNetwork::new(&map, 1.);
        let unit = graph.solve(1.);
        let pressure = 48. / unit.rates[map.pump_edge];
        let flow = graph.solve(pressure);
        let mut max_error = 0_f32;
        for chunk in map.edges[12..].chunks_exact(106) {
            for (radius, count, q) in [(12., 2, 48.), (6., 8, 12.), (3., 32, 3.), (1.5, 64, 0.75)] {
                assert_eq!(
                    chunk.iter().filter(|e| e.half_width == radius).count(),
                    count
                );
                let mut grade_error = 0_f32;
                let mut lengths = [f32::INFINITY, f32::NEG_INFINITY];
                for e in chunk.iter().filter(|e| e.half_width == radius) {
                    let index = map.edges.iter().position(|x| std::ptr::eq(x, e)).unwrap();
                    let error = (flow.rates[index] - q).abs();
                    max_error = max_error.max(error);
                    grade_error = grade_error.max(error);
                    lengths[0] = lengths[0].min(map.edge_length(index));
                    lengths[1] = lengths[1].max(map.edge_length(index));
                    assert!(
                        error < 0.0001,
                        "edge {index}: {} expected {q}",
                        flow.rates[index]
                    );
                }
                eprintln!(
                    "D{} count={count}, Q={q}, max_error={grade_error}, length_range={lengths:?}",
                    radius * 2.
                );
            }
        }
        for n in 0..map.nodes.len() {
            let residual: f32 = map
                .edges
                .iter()
                .zip(&flow.rates)
                .map(|(e, q)| {
                    if e.from == n {
                        *q
                    } else if e.to == n {
                        -q
                    } else {
                        0.
                    }
                })
                .sum();
            assert!(residual.abs() < 0.0001, "node {n}: {residual}");
        }
        eprintln!(
            "reference pressure={pressure}, Q_unit={}, max rate error={max_error}",
            unit.rates[map.pump_edge]
        );
        let mut bounds = [[f32::INFINITY; 2], [f32::NEG_INFINITY; 2]];
        for e in &map.edges {
            assert_eq!(e.centerline.first(), Some(&map.nodes[e.from]));
            assert_eq!(e.centerline.last(), Some(&map.nodes[e.to]));
            for p in &e.centerline {
                for j in 0..2 {
                    bounds[0][j] = bounds[0][j].min(p[j] - e.half_width);
                    bounds[1][j] = bounds[1][j].max(p[j] + e.half_width);
                }
            }
        }
        eprintln!("capsule bounds={bounds:?}");
        for pulmonary in [true, false] {
            let route = route(pulmonary);
            let length: f32 = route
                .windows(2)
                .map(|p| (p[1].1[0] - p[0].1[0]).hypot(p[1].1[1] - p[0].1[1]))
                .sum();
            eprintln!(
                "pulmonary={pulmonary}: selected route length={length}, waypoints={}",
                route.len()
            );
        }
    }

    fn segment_distance(a: Point, b: Point, c: Point, d: Point) -> f32 {
        fn cross(a: Point, b: Point, c: Point) -> f32 {
            (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
        }
        fn point(p: Point, a: Point, b: Point) -> f32 {
            let v = [b[0] - a[0], b[1] - a[1]];
            let t = (((p[0] - a[0]) * v[0] + (p[1] - a[1]) * v[1]) / (v[0] * v[0] + v[1] * v[1]))
                .clamp(0., 1.);
            (p[0] - a[0] - t * v[0]).hypot(p[1] - a[1] - t * v[1])
        }
        if cross(a, b, c) * cross(a, b, d) < 0. && cross(c, d, a) * cross(c, d, b) < 0. {
            0.
        } else {
            point(a, c, d)
                .min(point(b, c, d))
                .min(point(c, a, b))
                .min(point(d, a, b))
        }
    }

    #[test]
    fn heart_preview_external_tubes_and_domain_are_separate() {
        let map = MapDefinition::heart_preview();
        let mut minimum = f32::INFINITY;
        for i in 12..map.edges.len() {
            let a = &map.edges[i];
            for (j, b) in map.edges.iter().enumerate().skip(i + 1) {
                let shared: Vec<_> = [a.from, a.to]
                    .into_iter()
                    .filter(|n| *n == b.from || *n == b.to)
                    .collect();
                for x in a.centerline.windows(2) {
                    for y in b.centerline.windows(2) {
                        // Only the actual endpoint fans may share fluid space.
                        if shared.iter().any(|&n| {
                            let p = map.nodes[n];
                            let near = |q: Point| {
                                (q[0] - p[0]).hypot(q[1] - p[1])
                                    < 4. * a.half_width.max(b.half_width)
                            };
                            (near(x[0]) || near(x[1])) && (near(y[0]) || near(y[1]))
                        }) {
                            continue;
                        }
                        let gap =
                            segment_distance(x[0], x[1], y[0], y[1]) - a.half_width - b.half_width;
                        assert!(gap > 0., "external edges {i}/{j} overlap, gap={gap}");
                        minimum = minimum.min(gap);
                    }
                }
            }
            // Include capsule caps when checking the heart's future bake domain.
            for pair in a.centerline.windows(2) {
                let steps =
                    ((pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]) / 2.).ceil() as usize;
                for k in 0..=steps {
                    let t = k as f32 / steps.max(1) as f32;
                    let p: Point =
                        std::array::from_fn(|j| pair[0][j] + t * (pair[1][j] - pair[0][j]));
                    let dx = (-32. - p[0]).max(0.).max(p[0] - 224.);
                    let dy = (-32. - p[1]).max(0.).max(p[1] - 224.);
                    assert!(
                        dx.hypot(dy) > a.half_width,
                        "external edge {i} enters domain at {p:?}"
                    );
                }
            }
        }
        // Every microtube crosses the central seam independently; actual SDF
        // tissue must separate all neighboring lumens, in both modules.
        for center in [-800., 900.] {
            for i in 0..64 {
                let y = center - 504. + i as f32 * 16.;
                assert!(map.signed_distance(Vector::new(0., y)) < -1.4);
                if i < 63 {
                    assert!(map.signed_distance(Vector::new(0., y + 8.)) > 6.);
                }
            }
        }
        for (x, r) in [(38., 5.), (78., 4.5), (112., 5.), (150., 5.)] {
            assert!((map.signed_distance(Vector::new(x, -32.)) + r).abs() < 0.001);
        }
        for (_, p) in crate::map::heart_preview_route() {
            assert!(
                map.signed_distance(Vector::new(p[0], p[1])) < -0.75,
                "route point {p:?}"
            );
        }
        eprintln!("minimum external capsule gap={minimum}");
    }
}
