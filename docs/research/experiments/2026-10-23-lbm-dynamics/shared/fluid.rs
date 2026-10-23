//! Fixed-wall D2Q9 experiment. Coordinates and velocities use the unit physical domain.
const CX: [isize; 9] = [0, 1, 0, -1, 0, 1, -1, -1, 1];
const CY: [isize; 9] = [0, 0, 1, 0, -1, 1, 1, -1, -1];
const OPP: [usize; 9] = [0, 3, 4, 1, 2, 7, 8, 5, 6];
const W: [f64; 9] = [
    4.0 / 9.0,
    1.0 / 9.0,
    1.0 / 9.0,
    1.0 / 9.0,
    1.0 / 9.0,
    1.0 / 36.0,
    1.0 / 36.0,
    1.0 / 36.0,
    1.0 / 36.0,
];

#[derive(Clone, Copy, Debug)]
pub struct Marker {
    pub position: [f64; 2],
    pub velocity: [f64; 2],
    pub drag: f64,
}

#[derive(Debug)]
pub enum FluidError {
    InvalidMarker(usize),
    WallSupport(usize),
    Unstable,
}

/// Impulses are in physical units for one lattice step. Marker impulses act on the body.
pub struct StepExchange {
    pub marker_impulses: Vec<[f64; 2]>,
    pub fluid_impulse: [f64; 2],
    pub impulse_error: [f64; 2],
    pub max_kernel_weight_error: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Diagnostics {
    pub max_speed: f64,
    pub min_rho: f64,
    pub max_rho: f64,
    pub total_mass: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct CellField {
    pub position: [f64; 2],
    pub velocity: [f64; 2],
    pub rho: f64,
}

pub struct Fluid {
    n: usize,
    dx: f64,
    dt: f64,
    tau_inv: f64,
    guo_factor: f64,
    bg_lu: f64,
    f: Vec<[f64; 9]>,
    next: Vec<[f64; 9]>,
    force: Vec<[f64; 2]>,
    targets: Vec<[(usize, usize); 9]>,
}

fn equilibrium(rho: f64, ux: f64, uy: f64, k: usize) -> f64 {
    let cu = CX[k] as f64 * ux + CY[k] as f64 * uy;
    W[k] * rho * (1.0 + 3.0 * cu + 4.5 * cu * cu - 1.5 * (ux * ux + uy * uy))
}

fn macrostate(f: &[f64; 9], force: [f64; 2]) -> (f64, f64, f64) {
    let rho: f64 = f.iter().sum();
    let mut jx = 0.5 * force[0];
    let mut jy = 0.5 * force[1];
    for k in 0..9 {
        jx += f[k] * CX[k] as f64;
        jy += f[k] * CY[k] as f64;
    }
    (rho, jx / rho, jy / rho)
}

fn peskin_phi(r: f64) -> f64 {
    let a = r.abs();
    if a < 1.0 {
        (3.0 - 2.0 * a + (1.0 + 4.0 * a - 4.0 * a * a).sqrt()) / 8.0
    } else if a < 2.0 {
        (5.0 - 2.0 * a - (-7.0 + 12.0 * a - 4.0 * a * a).sqrt()) / 8.0
    } else {
        0.0
    }
}

impl Fluid {
    pub fn new(n: usize, tau: f64, background_force: f64) -> Self {
        assert!(n >= 4 && tau.is_finite() && tau > 0.5 && background_force.is_finite());
        let dx = 1.0 / n as f64;
        let dt = (tau - 0.5) / 3.0 * dx * dx; // physical kinematic viscosity = 1
        let tau_inv = 1.0 / tau;
        let mut targets = vec![[(0, 0); 9]; n * n];
        let mut written = vec![false; n * n * 9];
        for y in 0..n {
            for x in 0..n {
                let i = y * n + x;
                for k in 0..9 {
                    let yy = y as isize + CY[k];
                    let xx = (x as isize + CX[k]).rem_euclid(n as isize) as usize;
                    let dst = if yy < 0 || yy >= n as isize {
                        (i, OPP[k])
                    } else {
                        (yy as usize * n + xx, k)
                    };
                    assert!(!written[dst.0 * 9 + dst.1]);
                    written[dst.0 * 9 + dst.1] = true;
                    targets[i][k] = dst;
                }
            }
        }
        assert!(written.iter().all(|v| *v));
        Self {
            n,
            dx,
            dt,
            tau_inv,
            guo_factor: 1.0 - 0.5 * tau_inv,
            bg_lu: background_force * dt * dt / dx,
            f: vec![W; n * n],
            next: vec![[0.0; 9]; n * n],
            force: vec![[0.0; 2]; n * n],
            targets,
        }
    }

    pub fn dt(&self) -> f64 {
        self.dt
    }

    /// Uses previous half-force velocity for marker drag, then the new force in collision.
    pub fn advance(&mut self, markers: &[Marker]) -> Result<StepExchange, FluidError> {
        // Validate every support before changing the fluid, so a rejected step leaves it intact.
        let stencils: Vec<Vec<(usize, f64)>> = markers
            .iter()
            .enumerate()
            .map(|(m, marker)| {
                if !marker
                    .position
                    .iter()
                    .chain(marker.velocity.iter())
                    .all(|v| v.is_finite())
                    || !marker.drag.is_finite()
                    || marker.drag < 0.0
                {
                    return Err(FluidError::InvalidMarker(m));
                }
                let gx = marker.position[0] * self.n as f64 - 0.5;
                let gy = marker.position[1] * self.n as f64 - 0.5;
                if !gx.is_finite() || gx.abs() > isize::MAX as f64 / 2.0 || !gy.is_finite() {
                    return Err(FluidError::InvalidMarker(m));
                }
                let base_x = gx.floor() as isize;
                let base_y = gy.floor() as isize;
                let mut stencil = Vec::with_capacity(16);
                for yy in base_y - 1..=base_y + 2 {
                    let wy = peskin_phi(yy as f64 - gy);
                    if wy == 0.0 {
                        continue;
                    }
                    if yy < 0 || yy >= self.n as isize {
                        return Err(FluidError::WallSupport(m));
                    }
                    for xx in base_x - 1..=base_x + 2 {
                        let w = peskin_phi(xx as f64 - gx) * wy;
                        if w > 0.0 {
                            let x = xx.rem_euclid(self.n as isize) as usize;
                            stencil.push((yy as usize * self.n + x, w));
                        }
                    }
                }
                Ok(stencil)
            })
            .collect::<Result<_, _>>()?;

        let mut marker_forces = Vec::with_capacity(markers.len());
        let mut marker_impulses = Vec::with_capacity(markers.len());
        let mut max_kernel_weight_error: f64 = 0.0;
        for (marker, stencil) in markers.iter().zip(stencils.iter()) {
            let mut ju = [0.0; 2];
            let mut weight_sum = 0.0;
            for &(i, w) in stencil {
                let (_, ux, uy) = macrostate(&self.f[i], self.force[i]);
                ju[0] += w * ux * self.dx / self.dt;
                ju[1] += w * uy * self.dx / self.dt;
                weight_sum += w;
            }
            max_kernel_weight_error = max_kernel_weight_error.max((1.0 - weight_sum).abs());
            let f = [
                marker.drag * (marker.velocity[0] - ju[0]),
                marker.drag * (marker.velocity[1] - ju[1]),
            ];
            marker_forces.push(f);
            marker_impulses.push([-f[0] * self.dt, -f[1] * self.dt]);
        }
        self.force.fill([self.bg_lu, 0.0]);
        let scatter_scale = self.dt * self.dt / (self.dx * self.dx * self.dx);
        for (stencil, f) in stencils.iter().zip(marker_forces.iter()) {
            for &(i, w) in stencil {
                self.force[i][0] += scatter_scale * w * f[0];
                self.force[i][1] += scatter_scale * w * f[1];
            }
        }
        let impulse_scale = self.dx * self.dx * self.dx / self.dt;
        let mut fluid_impulse = [0.0; 2];
        for force in &self.force {
            fluid_impulse[0] += (force[0] - self.bg_lu) * impulse_scale;
            fluid_impulse[1] += force[1] * impulse_scale;
        }
        for i in 0..self.n * self.n {
            let (rho, ux, uy) = macrostate(&self.f[i], self.force[i]);
            if !rho.is_finite() || rho <= 0.0 || !ux.is_finite() || !uy.is_finite() {
                return Err(FluidError::Unstable);
            }
            for k in 0..9 {
                let cx = CX[k] as f64;
                let cy = CY[k] as f64;
                let cu = cx * ux + cy * uy;
                let guo = W[k]
                    * self.guo_factor
                    * ((3.0 * (cx - ux) + 9.0 * cu * cx) * self.force[i][0]
                        + (3.0 * (cy - uy) + 9.0 * cu * cy) * self.force[i][1]);
                let post = self.f[i][k]
                    - (self.f[i][k] - equilibrium(rho, ux, uy, k)) * self.tau_inv
                    + guo;
                if !post.is_finite() {
                    return Err(FluidError::Unstable);
                }
                let (dst, q) = self.targets[i][k];
                self.next[dst][q] = post;
            }
        }
        std::mem::swap(&mut self.f, &mut self.next);
        let mut impulse_error = fluid_impulse;
        for p in &marker_impulses {
            impulse_error[0] += p[0];
            impulse_error[1] += p[1];
        }
        Ok(StepExchange {
            marker_impulses,
            fluid_impulse,
            impulse_error,
            max_kernel_weight_error,
        })
    }

    pub fn diagnostics(&self) -> Diagnostics {
        let mut out = Diagnostics {
            max_speed: 0.0,
            min_rho: f64::INFINITY,
            max_rho: f64::NEG_INFINITY,
            total_mass: 0.0,
        };
        for i in 0..self.n * self.n {
            let (rho, ux, uy) = macrostate(&self.f[i], self.force[i]);
            out.total_mass += rho * self.dx * self.dx;
            out.min_rho = out.min_rho.min(rho);
            out.max_rho = out.max_rho.max(rho);
            out.max_speed = out.max_speed.max(ux.hypot(uy) * self.dx / self.dt);
        }
        out
    }

    pub fn field(&self) -> Vec<CellField> {
        (0..self.n * self.n)
            .map(|i| {
                let (rho, ux, uy) = macrostate(&self.f[i], self.force[i]);
                CellField {
                    position: [
                        ((i % self.n) as f64 + 0.5) * self.dx,
                        ((i / self.n) as f64 + 0.5) * self.dx,
                    ],
                    velocity: [ux * self.dx / self.dt, uy * self.dx / self.dt],
                    rho,
                }
            })
            .collect()
    }
}
