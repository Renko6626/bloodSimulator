//! Geometry-only constrained quadratic fit; no fallback for deficient neighborhoods.
use bytemuck::{Pod, Zeroable};
use faer::Mat;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(super) struct FitSample {
    pub index: u32,
    pub kind: u32,
    pub wn: f32,
    pub wt: f32,
}

pub(super) fn build(
    n: usize,
    solid: &[bool],
    boundary: &[[f32; 4]],
) -> Result<Vec<FitSample>, String> {
    let mut fit = vec![FitSample::default(); n * n];
    for i in 0..n * n {
        let normal = match boundary[i][0] as u32 {
            0 => continue,
            1 => [1, 0],
            2 => [-1, 0],
            3 => [0, 1],
            4 => [0, -1],
            _ => return Err("invalid opening side".into()),
        };
        let tangent = [-normal[1], normal[0]];
        let (x, y) = ((i % n) as i32, (i / n) as i32);
        let mut points = Vec::new();
        let mut rows = Vec::new();
        let mut sw = Vec::new();
        for dy in -4..=4 {
            for dx in -4..=4 {
                let r2 = dx * dx + dy * dy;
                if r2 == 0 || r2 > 16 || !valid(n, solid, x + dx, y + dy) {
                    continue;
                }
                let j = (y + dy) as usize * n + (x + dx) as usize;
                let kind = u32::from(boundary[j][0] != 0.);
                // Same side, not same opening ID; visibility also excludes separated vessels.
                if kind != 0 && boundary[j][0] != boundary[i][0] {
                    continue;
                }
                if !visible(n, solid, x, y, dx, dy) {
                    continue;
                }
                let s = f64::from(dx * normal[0] + dy * normal[1]) / 4.;
                let t = f64::from(dx * tangent[0] + dy * tangent[1]) / 4.;
                points.push((j, kind));
                rows.push([s, t, 0.5 * s * s, s * t, 0.5 * t * t]);
                sw.push(1. / f64::from(1 + r2).sqrt());
            }
        }
        let unsupported = || format!("unsupported LSQ neighborhood at ({x},{y})");
        if points.len() < 5 {
            return Err(format!("{}: fewer than five samples", unsupported()));
        }
        let a = Mat::from_fn(points.len(), 5, |r, c| rows[r][c] * sw[r]);
        let svd = a.as_ref().thin_svd().map_err(|_| unsupported())?;
        let largest = svd.S()[0];
        let smallest = svd.S()[4];
        if !largest.is_finite()
            || !smallest.is_finite()
            || smallest <= largest * 1e-12
            || largest / smallest > 1000.
        {
            return Err(format!("{}: rank/condition rejection", unsupported()));
        }
        let inverse = svd.pseudoinverse();
        fit[i] = FitSample {
            index: fit.len() as u32,
            kind: points.len() as u32,
            ..Default::default()
        };
        for (k, &(j, kind)) in points.iter().enumerate() {
            fit.push(FitSample {
                index: j as u32,
                kind,
                wn: (inverse[(0, k)] * sw[k] / 4.) as f32,
                wt: (inverse[(1, k)] * sw[k] / 4.) as f32,
            });
        }
    }
    Ok(fit)
}
fn valid(n: usize, solid: &[bool], x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < n as i32 && y < n as i32 && !solid[y as usize * n + x as usize]
}
fn visible(n: usize, solid: &[bool], x: i32, y: i32, dx: i32, dy: i32) -> bool {
    let (mut cx, mut cy) = (x, y);
    let (sx, sy) = (dx.signum(), dy.signum());
    let stepx = if dx == 0 {
        f64::INFINITY
    } else {
        1. / f64::from(dx.abs())
    };
    let stepy = if dy == 0 {
        f64::INFINITY
    } else {
        1. / f64::from(dy.abs())
    };
    let (mut tx, mut ty) = (0.5 * stepx, 0.5 * stepy);
    while (cx, cy) != (x + dx, y + dy) {
        if (tx - ty).abs() < 1e-12 {
            if !valid(n, solid, cx + sx, cy) || !valid(n, solid, cx, cy + sy) {
                return false;
            }
            cx += sx;
            cy += sy;
            tx += stepx;
            ty += stepy;
        } else if tx < ty {
            cx += sx;
            tx += stepx;
        } else {
            cy += sy;
            ty += stepy;
        }
        if !valid(n, solid, cx, cy) {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quadratic_gradients_and_wall_visibility() {
        let n = 12;
        let mut solid = vec![false; n * n];
        let mut boundary = vec![[0.; 4]; n * n];
        let i = 5 * n;
        boundary[i][0] = 1.;
        // Both velocity components, linear and quadratic fields, all normal orientations.
        for (cell, side, normal) in [
            (5 * n, 1., [1., 0.]),
            (6 * n - 1, 2., [-1., 0.]),
            (5, 3., [0., 1.]),
            (11 * n + 5, 4., [0., -1.]),
        ] {
            let mut sides = vec![[0.; 4]; n * n];
            sides[cell][0] = side;
            let samples = build(n, &solid, &sides).unwrap();
            let h = samples[cell];
            let tangent = [-normal[1], normal[0]];
            for quadratic in [false, true] {
                let mut gradient = [[0_f64; 2]; 2];
                for s in &samples[h.index as usize..(h.index + h.kind) as usize] {
                    let x = (s.index as usize % n) as f64 - (cell % n) as f64;
                    let y = (s.index as usize / n) as f64 - (cell / n) as f64;
                    let du = [3. * x - 4. * y, -2. * x + 5. * y];
                    let curvature = if quadratic {
                        [
                            0.7 * x * x + 0.2 * x * y - 0.3 * y * y,
                            -0.2 * x * x + 0.5 * x * y + 0.4 * y * y,
                        ]
                    } else {
                        [0.; 2]
                    };
                    for k in 0..2 {
                        gradient[0][k] += f64::from(s.wn) * (du[k] + curvature[k]);
                        gradient[1][k] += f64::from(s.wt) * (du[k] + curvature[k]);
                    }
                }
                for (k, cartesian) in [[3., -4.], [-2., 5.]].iter().enumerate() {
                    assert!(
                        (gradient[0][k] - cartesian[0] * normal[0] - cartesian[1] * normal[1])
                            .abs()
                            < 1e-6
                    );
                    assert!(
                        (gradient[1][k] - cartesian[0] * tangent[0] - cartesian[1] * tangent[1])
                            .abs()
                            < 1e-6
                    );
                }
            }
        }
        // A diagonal touching a solid corner is excluded, even if its endpoint is fluid.
        solid[5 * n + 1] = true;
        assert!(!visible(n, &solid, 0, 5, 2, 2));
        let fit = build(n, &solid, &boundary).unwrap();
        let h = fit[i];
        assert!(
            !fit[h.index as usize..(h.index + h.kind) as usize]
                .iter()
                .any(|s| s.index as usize == 7 * n + 2)
        );
    }
    #[test]
    fn rejects_rank_deficient_neighborhood() {
        let n = 12;
        let mut solid = vec![true; n * n];
        for x in 0..n {
            solid[5 * n + x] = false;
        }
        let mut boundary = vec![[0.; 4]; n * n];
        boundary[5 * n][0] = 1.;
        assert!(
            build(n, &solid, &boundary)
                .unwrap_err()
                .contains("fewer than five samples")
        );
        // Eight collinear samples: enough rows, but missing transverse derivatives.
        boundary[5 * n][0] = 0.;
        boundary[5 * n + 5][0] = 1.;
        assert!(
            build(n, &solid, &boundary)
                .unwrap_err()
                .contains("rank/condition rejection")
        );
    }
}
