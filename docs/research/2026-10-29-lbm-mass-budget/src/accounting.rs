use crate::lbm::{WindowGeometry, WindowSide};
pub const C: [[i32; 2]; 9] = [
    [0, 0],
    [1, 0],
    [0, 1],
    [-1, 0],
    [0, -1],
    [1, 1],
    [-1, 1],
    [-1, -1],
    [1, -1],
];
const W: [f64; 9] = [
    4. / 9.,
    1. / 9.,
    1. / 9.,
    1. / 9.,
    1. / 9.,
    1. / 36.,
    1. / 36.,
    1. / 36.,
    1. / 36.,
];
const OPP: [usize; 9] = [0, 3, 4, 1, 2, 7, 8, 5, 6];
#[derive(Clone, Copy, Default)]
pub struct Totals {
    pub shifted: f64,
    pub lo: f64,
    pub hi: f64,
    pub max_lattice: f64,
}
pub fn totals(g: &[f32], mask: &[bool]) -> Totals {
    let n = mask.len();
    let mut r = Totals {
        lo: f64::INFINITY,
        hi: f64::NEG_INFINITY,
        ..Default::default()
    };
    for i in 0..n {
        if mask[i] {
            continue;
        }
        let mut dr = 0.;
        let mut j = [0.; 2];
        for q in 0..9 {
            let f = f64::from(g[q * n + i]);
            dr += f;
            j[0] += f * C[q][0] as f64;
            j[1] += f * C[q][1] as f64;
        }
        let rho = 1. + dr;
        r.shifted += dr;
        r.lo = r.lo.min(rho);
        r.hi = r.hi.max(rho);
        r.max_lattice = r.max_lattice.max(j[0].hypot(j[1]) / rho);
    }
    r
}
#[derive(Default)]
pub struct OpeningBudget {
    pub incoming_delta: f64,
    pub known_delta: f64,
    pub tangent_delta: f64,
    pub lin: f64,
    pub lout: f64,
    pub b1: f64,
    pub c1: f64,
    pub jn2: f64,
    pub q0: f64,
    pub q2: f64,
    pub j0: f64,
    pub j2: f64,
    pub placeholder_max: f64,
    pub placeholder_mismatches: usize,
}
pub fn opening_budgets(raw: &[f32], geo: &WindowGeometry, h: f64, dt: f64) -> Vec<OpeningBudget> {
    let count = geo.solid.len();
    let n = (count as f64).sqrt() as usize;
    let g0 = &raw[..9 * count];
    let g1 = &raw[9 * count..18 * count];
    let g2 = &raw[18 * count..27 * count];
    let post = &raw[27 * count..];
    geo.openings
        .iter()
        .map(|o| {
            let (normal, tangent) = match o.side {
                WindowSide::Left => ([1, 0], [0, 1]),
                WindowSide::Right => ([-1, 0], [0, -1]),
                WindowSide::Bottom => ([0, 1], [-1, 0]),
                WindowSide::Top => ([0, -1], [1, 0]),
            };
            let mut r = OpeningBudget::default();
            for c in o.cells.clone() {
                let i = match o.side {
                    WindowSide::Left => c as usize * n,
                    WindowSide::Right => c as usize * n + n - 1,
                    WindowSide::Bottom => c as usize,
                    WindowSide::Top => (n - 1) * n + c as usize,
                };
                let (mut dr0, mut dr2, mut jn0, mut jn2) = (0., 0., 0., 0.);
                for q in 0..9 {
                    let a = f64::from(g0[q * count + i]);
                    let b = f64::from(g1[q * count + i]);
                    let z = f64::from(g2[q * count + i]);
                    let cn = C[q][0] * normal[0] + C[q][1] * normal[1];
                    let change = z - b;
                    dr0 += a;
                    dr2 += z;
                    jn0 += a * cn as f64;
                    jn2 += z * cn as f64;
                    match cn {
                        1 => {
                            r.incoming_delta += change;
                            r.c1 += b + W[q];
                            r.lin += z + W[q];
                        }
                        -1 => {
                            r.known_delta += change;
                            r.b1 += b + W[q];
                            let ct = C[q][0] * tangent[0] + C[q][1] * tangent[1];
                            let actual = post[3 * i + (ct + 1) as usize];
                            let placeholder = g1[OPP[q] * count + i];
                            r.lout += f64::from(actual) + W[q];
                            r.placeholder_max = r
                                .placeholder_max
                                .max((f64::from(actual) - f64::from(placeholder)).abs());
                            r.placeholder_mismatches +=
                                usize::from(actual.to_bits() != placeholder.to_bits());
                        }
                        _ => {
                            r.tangent_delta += change;
                        }
                    }
                }
                let scale = h * h / dt; // D=1; outward is -normal.
                r.q0 -= jn0 / (1. + dr0) * scale;
                r.q2 -= jn2 / (1. + dr2) * scale;
                r.j0 -= jn0 * scale;
                r.j2 -= jn2 * scale;
                r.jn2 += jn2;
            }
            r
        })
        .collect()
}
pub fn checksum(values: &[f32]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for v in values {
        for b in v.to_bits().to_le_bytes() {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}
