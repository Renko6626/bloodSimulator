use super::{MapDefinition, MapGeometry};
use rapier2d::prelude::Vector;

fn crossing(a: [f32; 2], b: [f32; 2], fa: f32, fb: f32) -> [f32; 2] {
    let t = fa / (fa - fb);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn polygons(mask: u8, center_inside: bool) -> &'static [&'static [u8]] {
    // Corners 0..3 run clockwise from the upper left; 4..7 are their following edges.
    // At a saddle the sampled center decides which pair of crossings is connected.
    match (mask, center_inside) {
        (0, _) => &[],
        (1, _) => &[&[0, 4, 7]],
        (2, _) => &[&[1, 5, 4]],
        (3, _) => &[&[0, 1, 5, 7]],
        (4, _) => &[&[2, 6, 5]],
        (5, false) => &[&[0, 4, 7], &[2, 6, 5]],
        (5, true) => &[&[0, 4, 5, 2, 6, 7]],
        (6, _) => &[&[1, 2, 6, 4]],
        (7, _) => &[&[0, 1, 2, 6, 7]],
        (8, _) => &[&[3, 7, 6]],
        (9, _) => &[&[0, 4, 6, 3]],
        (10, false) => &[&[1, 5, 4], &[3, 7, 6]],
        (10, true) => &[&[4, 1, 5, 6, 3, 7]],
        (11, _) => &[&[0, 1, 5, 6, 3]],
        (12, _) => &[&[2, 3, 7, 5]],
        (13, _) => &[&[0, 4, 5, 2, 3]],
        (14, _) => &[&[4, 1, 2, 3, 7]],
        (15, _) => &[&[0, 1, 2, 3]],
        _ => unreachable!(),
    }
}

pub(super) fn build(map: &MapDefinition, spacing: f32) -> MapGeometry {
    assert!(
        spacing.is_finite() && spacing > 0.0,
        "grid spacing must be positive"
    );
    let width = map
        .edges
        .iter()
        .map(|edge| edge.half_width)
        .fold(0.0, f32::max);
    let pad = width + 2.0 * spacing;
    let min = [
        (map.edges
            .iter()
            .flat_map(|edge| edge.centerline.iter())
            .map(|p| p[0])
            .fold(f32::INFINITY, f32::min)
            - pad)
            / spacing,
        (map.edges
            .iter()
            .flat_map(|edge| edge.centerline.iter())
            .map(|p| p[1])
            .fold(f32::INFINITY, f32::min)
            - pad)
            / spacing,
    ]
    .map(|x| x.floor() * spacing);
    let max = [
        (map.edges
            .iter()
            .flat_map(|edge| edge.centerline.iter())
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max)
            + pad)
            / spacing,
        (map.edges
            .iter()
            .flat_map(|edge| edge.centerline.iter())
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max)
            + pad)
            / spacing,
    ]
    .map(|x| x.ceil() * spacing);
    let nx = ((max[0] - min[0]) / spacing).round() as usize + 1;
    let ny = ((max[1] - min[1]) / spacing).round() as usize + 1;
    let index = |x: usize, y: usize| y * nx + x;
    let point = |x: usize, y: usize| [min[0] + x as f32 * spacing, min[1] + y as f32 * spacing];

    let mut values = Vec::with_capacity(nx * ny);
    for y in 0..ny {
        for x in 0..nx {
            let p = point(x, y);
            values.push(map.signed_distance(Vector::new(p[0], p[1])));
        }
    }

    // The same cached crossing coordinates feed both collision boundary and floor triangles.
    let mut horizontal = vec![None; (nx - 1) * ny];
    let mut vertical = vec![None; nx * (ny - 1)];
    for y in 0..ny {
        for x in 0..nx - 1 {
            let fa = values[index(x, y)];
            let fb = values[index(x + 1, y)];
            if (fa <= 0.0) != (fb <= 0.0) {
                horizontal[y * (nx - 1) + x] = Some(crossing(point(x, y), point(x + 1, y), fa, fb));
            }
        }
    }
    for y in 0..ny - 1 {
        for x in 0..nx {
            let fa = values[index(x, y)];
            let fb = values[index(x, y + 1)];
            if (fa <= 0.0) != (fb <= 0.0) {
                vertical[y * nx + x] = Some(crossing(point(x, y), point(x, y + 1), fa, fb));
            }
        }
    }

    let mut walls = Vec::new();
    let mut floor_vertices = Vec::new();
    for y in 0..ny - 1 {
        for x in 0..nx - 1 {
            let corners = [
                point(x, y),
                point(x + 1, y),
                point(x + 1, y + 1),
                point(x, y + 1),
            ];
            let samples = [
                values[index(x, y)],
                values[index(x + 1, y)],
                values[index(x + 1, y + 1)],
                values[index(x, y + 1)],
            ];
            let mask = samples.iter().enumerate().fold(0_u8, |bits, (i, sample)| {
                bits | (u8::from(*sample <= 0.0) << i)
            });
            if mask == 0 {
                continue;
            }
            let center_inside = if mask == 5 || mask == 10 {
                let p = [corners[0][0] + spacing * 0.5, corners[0][1] + spacing * 0.5];
                map.signed_distance(Vector::new(p[0], p[1])) <= 0.0
            } else {
                false
            };
            let edges = [
                horizontal[y * (nx - 1) + x],
                vertical[y * nx + x + 1],
                horizontal[(y + 1) * (nx - 1) + x],
                vertical[y * nx + x],
            ];
            for polygon in polygons(mask, center_inside) {
                let vertex = |id: u8| -> [f32; 2] {
                    if id < 4 {
                        corners[id as usize]
                    } else {
                        edges[id as usize - 4].expect("marching-squares crossing")
                    }
                };
                for i in 0..polygon.len() {
                    let a = polygon[i];
                    let b = polygon[(i + 1) % polygon.len()];
                    if a >= 4 && b >= 4 {
                        walls.push(vec![vertex(a), vertex(b)]);
                    }
                }
                if center_inside && (mask == 5 || mask == 10) {
                    let center = [corners[0][0] + spacing * 0.5, corners[0][1] + spacing * 0.5];
                    for i in 0..polygon.len() {
                        floor_vertices.extend([
                            center,
                            vertex(polygon[i]),
                            vertex(polygon[(i + 1) % polygon.len()]),
                        ]);
                    }
                } else {
                    for i in 1..polygon.len() - 1 {
                        floor_vertices.extend([
                            vertex(polygon[0]),
                            vertex(polygon[i]),
                            vertex(polygon[i + 1]),
                        ]);
                    }
                }
            }
        }
    }
    MapGeometry {
        walls,
        floor_vertices,
        bounds: [min, max],
    }
}
