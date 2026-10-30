use super::{ChamberGeometry, Landmark, MapDefinition, VesselEdge};

// Standard cubic sampling, independent of the legacy vessel Bezier helper.
fn outline(name: &str, start: [f32; 2], curves: &[[[f32; 2]; 3]]) -> ChamberGeometry {
    let mut points = vec![start];
    let mut a = start;
    for &[b, c, d] in curves {
        for i in 1..=32 {
            let t = i as f32 / 32.;
            let u = 1. - t;
            points.push(std::array::from_fn(|j| {
                u * u * u * a[j] + 3. * u * u * t * b[j] + 3. * u * t * t * c[j] + t * t * t * d[j]
            }));
        }
        a = d;
    }
    ChamberGeometry::new(name, points)
}

/// Ordered input-replay waypoints, including the external pulmonary return.
pub fn heart_preview_route() -> Vec<(&'static str, [f32; 2])> {
    vec![
        ("tricuspid_approach", [50., 62.]),
        ("right_atrium", [45., 45.]),
        ("tricuspid", [49., 78.]),
        ("right_entry", [49., 86.]),
        ("right_deep", [61., 170.]),
        ("right_return", [62., 100.]),
        ("pulmonary_valve", [69., 85.]),
        ("pulmonary_turn", [78., 65.]),
        ("pulmonary_outlet", [78., 55.]),
        ("pulmonary_interface", [78., -12.]),
        ("lung_return", [112., -12.]),
        ("left_atrium", [112., 45.]),
        ("mitral", [106., 78.]),
        ("left_entry", [106., 86.]),
        ("left_deep", [106., 176.]),
        ("left_return", [123., 106.]),
        ("aortic_valve", [132., 86.]),
        ("aortic_turn", [150., 65.]),
        ("aortic_interface", [150., -4.]),
    ]
}

impl MapDefinition {
    pub fn heart_preview() -> Self {
        let chambers = vec![
            outline(
                "right_atrium",
                [24., 31.],
                &[
                    [[37., 20.], [60., 23.], [69., 34.]],
                    [[77., 47.], [70., 62.], [59., 67.]],
                    [[45., 72.], [26., 65.], [20., 55.]],
                    [[15., 46.], [17., 38.], [24., 31.]],
                ],
            ),
            outline(
                "left_atrium",
                [94., 28.],
                &[
                    [[108., 21.], [128., 26.], [135., 37.]],
                    [[142., 50.], [132., 62.], [119., 67.]],
                    [[106., 71.], [91., 61.], [87., 49.]],
                    [[83., 40.], [86., 32.], [94., 28.]],
                ],
            ),
            outline(
                "right_ventricle",
                [43., 78.],
                &[
                    [[57., 72.], [72., 80.], [76., 98.]],
                    [[81., 125.], [73., 158.], [64., 181.]],
                    [[43., 164.], [26., 133.], [22., 108.]],
                    [[18., 91.], [28., 82.], [43., 78.]],
                ],
            ),
            outline(
                "left_ventricle",
                [96., 77.],
                &[
                    [[111., 70.], [131., 79.], [137., 96.]],
                    [[144., 126.], [122., 166.], [105., 189.]],
                    [[92., 170.], [84., 137.], [86., 111.]],
                    [[86., 94.], [87., 83.], [96., 77.]],
                ],
            ),
        ];
        // One connected circulation cycle. All intracardiac guides stay within
        // their own chamber; only the pulmonary/body returns join the two sides.
        let nodes = vec![
            [38., -4.],
            [45., 45.],
            [49., 86.],
            [69., 85.],
            [78., -4.],
            [112., -4.],
            [112., 45.],
            [106., 86.],
            [132., 86.],
            [150., -4.],
        ];
        let paths = vec![
            vec![nodes[0], [38., 32.], nodes[1]],
            vec![nodes[1], [50., 62.], [49., 78.], nodes[2]],
            vec![nodes[2], [49., 110.], [61., 170.], [62., 100.], nodes[3]],
            vec![nodes[3], [78., 65.], [78., 55.], nodes[4]],
            vec![nodes[4], [78., -12.], [112., -12.], nodes[5]],
            vec![nodes[5], nodes[6]],
            vec![nodes[6], [108., 63.], [106., 78.], nodes[7]],
            vec![nodes[7], [106., 176.], [123., 106.], nodes[8]],
            vec![nodes[8], [150., 65.], nodes[9]],
            vec![
                nodes[9],
                [166., -4.],
                [166., 204.],
                [-6., 204.],
                [-6., -4.],
                nodes[0],
            ],
        ];
        let edges = paths
            .into_iter()
            .enumerate()
            .map(|(i, centerline)| VesselEdge {
                from: i,
                to: (i + 1) % nodes.len(),
                centerline,
                half_width: if i == 3 { 4.5 } else { 5. },
            })
            .collect();
        let landmarks = heart_preview_route()
            .into_iter()
            .map(|(name, position)| Landmark {
                name: name.into(),
                position,
                size: [4., 2.],
            })
            .collect();
        Self {
            fluid: None,
            nodes,
            edges,
            chambers,
            pump_edge: 9,
            spawn: [49., 72.],
            landmarks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rapier2d::prelude::Vector;
    use std::collections::{BTreeSet, VecDeque};
    fn flood(map: &MapDefinition, start: (i32, i32)) -> BTreeSet<(i32, i32)> {
        let mut seen = BTreeSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some((x, y)) = queue.pop_front() {
            for p in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if p.0 < 0 || p.0 > 80 || p.1 < 0 || p.1 > 96 || seen.contains(&p) {
                    continue;
                }
                if map.signed_distance(Vector::new(p.0 as f32 * 2., p.1 as f32 * 2.)) < -0.8 {
                    seen.insert(p);
                    queue.push_back(p);
                }
            }
        }
        seen
    }
    #[test]
    fn heart_preview_chambers_valves_and_septum() {
        let map = MapDefinition::heart_preview();
        for p in [
            [45., 45.],
            [112., 45.],
            [49., 110.],
            [106., 130.],
            [49., 74.],
            [106., 74.],
        ] {
            assert!(map.signed_distance(Vector::new(p[0], p[1])) < -1.);
        }
        assert!(map.signed_distance(Vector::new(81., 125.)) > 0.);
        // Flood only the heart ROI: the normal pulmonary/body loops are outside.
        let seen = flood(&map, (24, 23));
        assert!(
            seen.contains(&(30, 80)),
            "right ventricle must connect to right atrium"
        );
        assert!(
            !seen.contains(&(56, 23)),
            "septum must isolate left atrium inside ROI"
        );
        assert!(
            !seen.contains(&(53, 65)),
            "septum must isolate left ventricle inside ROI"
        );
        let mut closed = map.clone();
        closed.edges = closed
            .edges
            .into_iter()
            .enumerate()
            .filter_map(|(i, e)| (!matches!(i, 1 | 6)).then_some(e))
            .collect();
        assert!(
            !flood(&closed, (24, 23)).contains(&(30, 80)),
            "right outflow must not bypass AV opening"
        );
        assert!(
            !flood(&closed, (56, 23)).contains(&(53, 65)),
            "left outflow must not bypass AV opening"
        );
    }
}
