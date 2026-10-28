use crate::lbm::{WindowGeometry, WindowSide};
const C: [[f64; 2]; 9] = [
    [0., 0.],
    [1., 0.],
    [0., 1.],
    [-1., 0.],
    [0., -1.],
    [1., 1.],
    [-1., 1.],
    [-1., -1.],
    [1., -1.],
];
#[derive(Clone, Copy, Default)]
pub struct Cell {
    pub rho: f64,
    pub u: [f64; 2],
    pub stress: [f64; 3],
}
pub struct Field {
    pub cells: Vec<Cell>,
    pub shifted: f64,
    pub mean: f64,
    pub lo: f64,
    pub hi: f64,
    pub umax: f64,
    pub count: usize,
}
pub fn observe(g: &[f32], mask: &[bool], force: [f64; 2]) -> Result<Field, String> {
    let n = mask.len();
    let mut f = Field {
        cells: vec![Cell::default(); n],
        shifted: 0.,
        mean: 0.,
        lo: f64::INFINITY,
        hi: f64::NEG_INFINITY,
        umax: 0.,
        count: 0,
    };
    for i in 0..n {
        if mask[i] {
            continue;
        }
        let (mut dr, mut j, mut pi) = (0., [0.; 2], [0.; 3]);
        for q in 0..9 {
            let x = f64::from(g[q * n + i]);
            dr += x;
            for k in 0..2 {
                j[k] += C[q][k] * x;
            }
            pi[0] += C[q][0] * C[q][0] * x;
            pi[1] += C[q][1] * C[q][1] * x;
            pi[2] += C[q][0] * C[q][1] * x;
        }
        let rho = 1. + dr;
        let u = [(j[0] + force[0] * 0.5) / rho, (j[1] + force[1] * 0.5) / rho];
        let stress = [
            pi[0] - dr / 3. - rho * u[0] * u[0] + u[0] * force[0],
            pi[1] - dr / 3. - rho * u[1] * u[1] + u[1] * force[1],
            pi[2] - rho * u[0] * u[1] + 0.5 * (u[0] * force[1] + u[1] * force[0]),
        ];
        if rho <= 0. || !rho.is_finite() || !u.iter().chain(stress.iter()).all(|v| v.is_finite()) {
            return Err(format!("invalid raw moments cell={i} rho={rho}"));
        }
        f.cells[i] = Cell { rho, u, stress };
        f.shifted += dr;
        f.count += 1;
        f.lo = f.lo.min(rho);
        f.hi = f.hi.max(rho);
        f.umax = f.umax.max(u[0].hypot(u[1]));
    }
    f.mean = 1. + f.shifted / f.count as f64;
    Ok(f)
}
pub fn correction_limits(a: &Field, b: &Field, mask: &[bool], delta: f32) -> (f64, f64, f64, bool) {
    let (mut density, mut velocity, mut stress) = (0_f64, 0_f64, 0_f64);
    for i in 0..mask.len() {
        if mask[i] {
            continue;
        }
        let x = a.cells[i];
        let y = b.cells[i];
        density = density.max((y.rho - x.rho + f64::from(delta)).abs());
        velocity = velocity.max((y.u[0] - x.u[0]).hypot(y.u[1] - x.u[1]));
        for k in 0..3 {
            stress = stress.max((y.stress[k] - x.stress[k]).abs());
        }
    }
    let pass = (b.mean - 1.).abs() <= 5e-7 && density <= 5e-7 && velocity <= 5e-7 && stress <= 5e-7;
    (density, velocity, stress, pass)
}
#[derive(Default)]
pub struct Difference {
    pub u_rel: f64,
    pub u_rms: f64,
    pub u_max: f64,
    pub rho_rel: f64,
    pub rho_rms: f64,
    pub rho_max: f64,
    pub stress_rel: f64,
    pub stress_rms: f64,
    pub stress_max: f64,
}
fn relative(d: f64, n: f64) -> f64 {
    if n > 0. {
        (d / n).sqrt()
    } else if d == 0. {
        0.
    } else {
        f64::INFINITY
    }
}
pub fn difference(a: &Field, b: &Field, mask: &[bool], side: usize, central: bool) -> Difference {
    let ids: Vec<_> = (0..mask.len())
        .filter(|&i| {
            !mask[i]
                && (!central
                    || (i % side >= side / 3
                        && i % side < 2 * side / 3
                        && i / side >= side / 3
                        && i / side < 2 * side / 3))
        })
        .collect();
    let count = ids.len() as f64;
    let ma = ids.iter().map(|&i| a.cells[i].rho).sum::<f64>() / count;
    let mb = ids.iter().map(|&i| b.cells[i].rho).sum::<f64>() / count;
    let (mut du, mut nu, mut dr, mut nr, mut ds, mut ns) = (0., 0., 0., 0., 0., 0.);
    let mut r = Difference::default();
    for i in ids {
        let x = a.cells[i];
        let y = b.cells[i];
        let u = (y.u[0] - x.u[0]).powi(2) + (y.u[1] - x.u[1]).powi(2);
        let rho = ((y.rho - mb) - (x.rho - ma)).abs();
        let st = (y.stress[0] - x.stress[0]).powi(2)
            + (y.stress[1] - x.stress[1]).powi(2)
            + 2. * (y.stress[2] - x.stress[2]).powi(2);
        du += u;
        nu += x.u[0].powi(2) + x.u[1].powi(2);
        dr += rho * rho;
        nr += (x.rho - ma).powi(2);
        ds += st;
        ns += x.stress[0].powi(2) + x.stress[1].powi(2) + 2. * x.stress[2].powi(2);
        r.u_max = r.u_max.max(u.sqrt());
        r.rho_max = r.rho_max.max(rho);
        r.stress_max = r.stress_max.max(st.sqrt());
    }
    r.u_rel = relative(du, nu);
    r.u_rms = (du / count).sqrt();
    r.rho_rel = relative(dr, nr);
    r.rho_rms = (dr / count).sqrt();
    r.stress_rel = relative(ds, ns);
    r.stress_rms = (ds / count).sqrt();
    r
}
pub fn boundary_errors(
    f: &Field,
    geometry: &WindowGeometry,
    profiles: &[Vec<f32>],
    targets: &[f32],
    h: f64,
    dt: f64,
) -> (f64, f64) {
    let n = (geometry.solid.len() as f64).sqrt() as usize;
    let (mut ue, mut qe) = (0_f64, 0_f64);
    for ((o, pf), target) in geometry.openings.iter().zip(profiles).zip(targets) {
        let normal = match o.side {
            WindowSide::Left => [-1., 0.],
            WindowSide::Right => [1., 0.],
            WindowSide::Bottom => [0., -1.],
            WindowSide::Top => [0., 1.],
        };
        let dot = f64::from(o.direction[0]) * normal[0] + f64::from(o.direction[1]) * normal[1];
        let mut q = 0.;
        for (c, v) in o.cells.clone().zip(pf) {
            let i = match o.side {
                WindowSide::Left => c as usize * n,
                WindowSide::Right => c as usize * n + n - 1,
                WindowSide::Bottom => c as usize,
                WindowSide::Top => (n - 1) * n + c as usize,
            };
            let u = f.cells[i].u;
            for k in 0..2 {
                ue =
                    ue.max((u[k] * h / dt - f64::from(o.direction[k]) * f64::from(*v) / dot).abs());
            }
            q += (u[0] * normal[0] + u[1] * normal[1]) * h * h / dt;
        }
        qe = qe.max((q - f64::from(*target)).abs());
    }
    (ue, qe)
}
