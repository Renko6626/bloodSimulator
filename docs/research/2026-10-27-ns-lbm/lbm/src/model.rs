use serde_json::{Value, json};
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
pub const W: [f64; 9] = [
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
pub const OPP: [usize; 9] = [0, 3, 4, 1, 2, 7, 8, 5, 6];
pub const M: [[f64; 9]; 9] = [
    [1., 1., 1., 1., 1., 1., 1., 1., 1.],
    [-4., -1., -1., -1., -1., 2., 2., 2., 2.],
    [4., -2., -2., -2., -2., 1., 1., 1., 1.],
    [0., 1., 0., -1., 0., 1., -1., -1., 1.],
    [0., -2., 0., 2., 0., 1., -1., -1., 1.],
    [0., 0., 1., 0., -1., 1., 1., -1., -1.],
    [0., 0., -2., 0., 2., 1., 1., -1., -1.],
    [0., 1., -1., 1., -1., 0., 0., 0., 0.],
    [0., 0., 0., 0., 0., 1., -1., 1., -1.],
];
pub const NORM: [f64; 9] = [9., 36., 36., 6., 12., 6., 12., 4., 4.];
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub nx: usize,
    pub ny: usize,
    pub lx: f64,
    pub ly: f64,
    pub h: f64,
    pub nu: f64,
    pub dt: f64,
    pub substeps: usize,
    pub periodic_y: bool,
}
impl Config {
    pub fn new(
        nx: usize,
        ny: usize,
        lx: f64,
        ly: f64,
        nu: f64,
        periodic_y: bool,
        velocity_bound: f64,
        refine: usize,
    ) -> Self {
        assert!(nx > 1 && ny > 1 && nu > 0. && velocity_bound > 0. && refine > 0);
        let h = lx / nx as f64;
        assert!((ly / ny as f64 - h).abs() < 1e-10);
        let limit = (0.05 * h / velocity_bound).min(0.5 * h * h / nu);
        let substeps = ((1. / 120.) / limit).ceil() as usize * refine;
        Self {
            nx,
            ny,
            lx,
            ly,
            h,
            nu,
            dt: 1. / (120. * substeps as f64),
            substeps,
            periodic_y,
        }
    }
    pub fn len(self) -> usize {
        self.nx * self.ny
    }
    pub fn tau(self) -> f64 {
        0.5 + 3. * self.nu * self.dt / (self.h * self.h)
    }
    pub fn position(self, i: usize) -> [f64; 2] {
        [
            ((i % self.nx) as f64 + 0.5) * self.h,
            ((i / self.nx) as f64 + 0.5) * self.h,
        ]
    }
    pub fn force(self, a: [f64; 2]) -> [f64; 2] {
        let k = self.dt * self.dt / self.h;
        [a[0] * k, a[1] * k]
    }
    pub fn metadata(self) -> Value {
        json!({"nx":self.nx,"ny":self.ny,"active_cells":self.len(),"lx":self.lx,"ly":self.ly,"h":self.h,"nu":self.nu,"dt_lbm":self.dt,"steps_per_mechanical":self.substeps,"steps_per_frame":self.substeps*2,"tau_nu":self.tau(),"periodic_y":self.periodic_y})
    }
}
pub fn equilibrium_delta(rho: f64, u: [f64; 2], q: usize) -> f64 {
    let cu = C[q][0] as f64 * u[0] + C[q][1] as f64 * u[1];
    W[q] * ((rho - 1.) + rho * (3. * cu + 4.5 * cu * cu - 1.5 * (u[0] * u[0] + u[1] * u[1])))
}
pub fn initialize(
    c: Config,
    velocity: impl Fn(f64, f64) -> [f64; 2],
    density: impl Fn(f64, f64) -> f64,
    a: [f64; 2],
) -> Vec<f64> {
    let mut f = vec![0.; 9 * c.len()];
    let force = c.force(a);
    for i in 0..c.len() {
        let [x, y] = c.position(i);
        let u = velocity(x, y);
        let u = [u[0] * c.dt / c.h, u[1] * c.dt / c.h];
        let rho = density(x, y);
        for q in 0..9 {
            f[q * c.len() + i] = equilibrium_delta(rho, u, q)
                - 1.5 * W[q] * (C[q][0] as f64 * force[0] + C[q][1] as f64 * force[1]);
        }
    }
    f
}
#[derive(Clone)]
pub struct Field {
    pub samples: Vec<[f64; 3]>,
}
impl Field {
    pub fn relative_error(
        &self,
        c: Config,
        exact: impl Fn(f64, f64) -> [f64; 2],
        translation: [f64; 2],
    ) -> f64 {
        let (mut e, mut d) = (0., 0.);
        for (i, p) in self.samples.iter().enumerate() {
            let [x, y] = c.position(i);
            let u = exact(x, y);
            for k in 0..2 {
                e += (p[k + 1] - u[k]).powi(2);
                d += (u[k] - translation[k]).powi(2);
            }
        }
        (e / d.max(1e-30)).sqrt()
    }
    pub fn diagnostics(&self, c: Config) -> Value {
        let (mut lo, mut hi, mut maxu, mut mx, mut my, mut rho, mut energy) =
            (f64::INFINITY, f64::NEG_INFINITY, 0_f64, 0., 0., 0., 0.);
        for p in &self.samples {
            assert!(
                p.iter().all(|x| x.is_finite()) && p[0] > 0.,
                "nonfinite or nonpositive fluid state"
            );
            lo = lo.min(p[0]);
            hi = hi.max(p[0]);
            maxu = maxu.max(p[1].hypot(p[2]));
            mx += p[1];
            my += p[2];
            rho += p[0];
            energy += 0.5 * p[0] * (p[1] * p[1] + p[2] * p[2]);
        }
        let n = c.len() as f64;
        json!({"rho_min":lo,"rho_max":hi,"rho_mean":rho/n,"mean_u":mx/n,"mean_v":my/n,"max_speed":maxu,"max_lattice_speed":maxu*c.dt/c.h,"max_mach":maxu*c.dt/c.h*3_f64.sqrt(),"re_measured_mean":(mx/n).abs()*c.ly/c.nu,"energy":energy*c.h*c.h})
    }
}
pub struct Cpu {
    pub c: Config,
    pub f: Vec<f64>,
    next: Vec<f64>,
}
impl Cpu {
    pub fn new(
        c: Config,
        u: impl Fn(f64, f64) -> [f64; 2],
        rho: impl Fn(f64, f64) -> f64,
        a: [f64; 2],
    ) -> Self {
        Self {
            c,
            f: initialize(c, u, rho, a),
            next: vec![0.; 9 * c.len()],
        }
    }
    pub fn kick(&mut self, kicks: &[[f64; 2]]) {
        let n = self.c.len();
        for (i, k) in kicks.iter().enumerate() {
            for (q, cv) in C.iter().enumerate() {
                self.f[q * n + i] += 3. * W[q] * (cv[0] as f64 * k[0] + cv[1] as f64 * k[1]);
            }
        }
    }
    pub fn field(&self, a: [f64; 2]) -> Field {
        let n = self.c.len();
        let force = self.c.force(a);
        let mut samples = Vec::with_capacity(n);
        for i in 0..n {
            let mut rho = 1.;
            let mut j = [0., 0.];
            for (q, cv) in C.iter().enumerate() {
                let f = self.f[q * n + i];
                rho += f;
                j[0] += cv[0] as f64 * f;
                j[1] += cv[1] as f64 * f;
            }
            samples.push([
                rho,
                (j[0] + 0.5 * force[0]) / rho * self.c.h / self.c.dt,
                (j[1] + 0.5 * force[1]) / rho * self.c.h / self.c.dt,
            ]);
        }
        Field { samples }
    }
    pub fn step(&mut self, a: [f64; 2]) {
        let c = self.c;
        let n = c.len();
        let force = c.force(a);
        let s = [0., 1.1, 1., 0., 1.2, 0., 1.2, 1. / c.tau(), 1. / c.tau()];
        for i in 0..n {
            let mut f = [0.; 9];
            let mut rho = 1.;
            let mut j = [0.; 2];
            for q in 0..9 {
                f[q] = self.f[q * n + i];
                rho += f[q];
                j[0] += C[q][0] as f64 * f[q];
                j[1] += C[q][1] as f64 * f[q];
            }
            let u = [(j[0] + 0.5 * force[0]) / rho, (j[1] + 0.5 * force[1]) / rho];
            let mut diff = [0.; 9];
            let mut source = [0.; 9];
            for q in 0..9 {
                let cv = [C[q][0] as f64, C[q][1] as f64];
                let cu = cv[0] * u[0] + cv[1] * u[1];
                diff[q] = f[q] - equilibrium_delta(rho, u, q);
                source[q] = W[q]
                    * ((3. * (cv[0] - u[0]) + 9. * cu * cv[0]) * force[0]
                        + (3. * (cv[1] - u[1]) + 9. * cu * cv[1]) * force[1]);
            }
            let mut dm = [0.; 9];
            for r in 0..9 {
                let d = (0..9).map(|q| M[r][q] * diff[q]).sum::<f64>();
                let z = (0..9).map(|q| M[r][q] * source[q]).sum::<f64>();
                dm[r] = -s[r] * d + (1. - 0.5 * s[r]) * z;
            }
            for q in 0..9 {
                let post = f[q] + (0..9).map(|r| M[r][q] / NORM[r] * dm[r]).sum::<f64>();
                let x = (i % c.nx) as i32 + C[q][0];
                let y = (i / c.nx) as i32 + C[q][1];
                if !c.periodic_y && (y < 0 || y >= c.ny as i32) {
                    self.next[OPP[q] * n + i] = post;
                } else {
                    let dest = y.rem_euclid(c.ny as i32) as usize * c.nx
                        + x.rem_euclid(c.nx as i32) as usize;
                    self.next[q * n + dest] = post;
                }
            }
        }
        std::mem::swap(&mut self.f, &mut self.next);
    }
}
