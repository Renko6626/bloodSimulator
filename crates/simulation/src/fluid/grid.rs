//! Stair-step MAC Stokes domain with pressure-traction openings.
//! A is assembled from viscous energy; boundary-normal half cells carry half
//! tangential energy. D and -D^T share the same face fluxes (cell area scaled out).
#[cfg(test)]
use blood_fluid::cpu::LuFactory;
use blood_fluid::{CsrMatrix, Geometry, PreparedSolver, SolverFactory, StokesProblem};
use std::collections::VecDeque;
use std::sync::Arc;

const NONE: usize = usize::MAX;

struct Face {
    position: [f64; 2],
    minus: usize,
    plus: usize,
}

struct Component {
    width: usize,
    height: usize,
    lookup: Vec<usize>,
    faces: Vec<Face>,
    diagonal: Vec<f64>,
    neighbors: Vec<Vec<(usize, f64)>>,
}

pub(super) struct Grid {
    pub n: usize,
    pub h: f64,
    pub origin: [f64; 2],
    pub cells: Vec<usize>,
    components: [Component; 2],
    factor: Box<dyn PreparedSolver>,
}

pub(super) struct Field {
    pub velocity: Vec<f64>,
    pub pressure: Vec<f64>,
    pub divergence_rms: f64,
    pub momentum_rms: f64,
    pub net_flux: f64,
    pub iterations: usize,
}

impl Grid {
    #[cfg(test)]
    pub fn new(
        n: usize,
        h: f64,
        origin: [f64; 2],
        mask: Vec<bool>,
        viscosity: f64,
        openings: [bool; 4],
    ) -> Result<Self, String> {
        Self::with_factory(n, h, origin, mask, viscosity, openings, &LuFactory)
    }

    pub fn with_factory(
        n: usize,
        h: f64,
        origin: [f64; 2],
        mask: Vec<bool>,
        viscosity: f64,
        openings: [bool; 4],
        factory: &dyn SolverFactory,
    ) -> Result<Self, String> {
        if n < 2
            || mask.len() != n * n
            || !h.is_finite()
            || h <= 0.0
            || !viscosity.is_finite()
            || viscosity <= 0.0
        {
            return Err("invalid fixed fluid grid".into());
        }
        let mut visited = vec![false; n * n];
        for start in 0..n * n {
            if !mask[start] || visited[start] {
                continue;
            }
            let mut queue = VecDeque::from([start]);
            visited[start] = true;
            let mut open = false;
            while let Some(k) = queue.pop_front() {
                let (i, j) = (k % n, k / n);
                open |= (i == 0 && openings[0])
                    || (i + 1 == n && openings[1])
                    || (j == 0 && openings[2])
                    || (j + 1 == n && openings[3]);
                for (x, y) in [
                    (i as isize - 1, j as isize),
                    (i as isize + 1, j as isize),
                    (i as isize, j as isize - 1),
                    (i as isize, j as isize + 1),
                ] {
                    if x < 0 || y < 0 || x >= n as isize || y >= n as isize {
                        continue;
                    }
                    let q = y as usize * n + x as usize;
                    if mask[q] && !visited[q] {
                        visited[q] = true;
                        queue.push_back(q);
                    }
                }
            }
            if !open {
                return Err("fixed fluid domain contains a component without an opening".into());
            }
        }
        let cells: Vec<_> = (0..n * n).filter(|&k| mask[k]).collect();
        if cells.is_empty() {
            return Err("fixed fluid domain is empty".into());
        }
        let mut pressures = vec![NONE; n * n];
        for (p, &k) in cells.iter().enumerate() {
            pressures[k] = p;
        }
        let build = |axis: usize| -> Result<Component, String> {
            let (width, height) = if axis == 0 { (n + 1, n) } else { (n, n + 1) };
            let mut lookup = vec![NONE; width * height];
            let mut faces = Vec::new();
            let mut coords = Vec::new();
            for j in 0..height {
                for i in 0..width {
                    let (minus, plus, allow) = if axis == 0 {
                        (
                            if i > 0 {
                                pressures[j * n + i - 1]
                            } else {
                                NONE
                            },
                            if i < n { pressures[j * n + i] } else { NONE },
                            if i == 0 {
                                openings[0]
                            } else if i == n {
                                openings[1]
                            } else {
                                false
                            },
                        )
                    } else {
                        (
                            if j > 0 {
                                pressures[(j - 1) * n + i]
                            } else {
                                NONE
                            },
                            if j < n { pressures[j * n + i] } else { NONE },
                            if j == 0 {
                                openings[2]
                            } else if j == n {
                                openings[3]
                            } else {
                                false
                            },
                        )
                    };
                    if (minus != NONE && plus != NONE) || (allow && (minus != NONE || plus != NONE))
                    {
                        lookup[j * width + i] = faces.len();
                        coords.push((i, j));
                        faces.push(Face {
                            minus,
                            plus,
                            position: [
                                origin[0] + (i as f64 + if axis == 0 { 0.0 } else { 0.5 }) * h,
                                origin[1] + (j as f64 + if axis == 1 { 0.0 } else { 0.5 }) * h,
                            ],
                        });
                    }
                }
            }
            let mut diagonal = vec![0.0; faces.len()];
            let mut neighbors = vec![Vec::new(); faces.len()];
            let coefficient = viscosity / (h * h);
            for (id, &(i, j)) in coords.iter().enumerate() {
                let half = if (axis == 0 && (i == 0 || i == n)) || (axis == 1 && (j == 0 || j == n))
                {
                    0.5
                } else {
                    1.0
                };
                for (direction, (dx, dy)) in
                    [(-1, 0), (1, 0), (0, -1), (0, 1)].into_iter().enumerate()
                {
                    let (x, y) = (i as isize + dx, j as isize + dy);
                    let normal = if axis == 0 { dy == 0 } else { dx == 0 };
                    let weight = if normal { 1.0 } else { half };
                    if x < 0 || y < 0 || x >= width as isize || y >= height as isize {
                        if !openings[direction] {
                            diagonal[id] += 2.0 * weight * coefficient;
                        }
                    } else {
                        let other = lookup[y as usize * width + x as usize];
                        if other != NONE {
                            diagonal[id] += weight * coefficient;
                            neighbors[id].push((other, -weight * coefficient));
                        } else {
                            diagonal[id] += if normal {
                                weight * coefficient
                            } else {
                                2.0 * weight * coefficient
                            };
                        }
                    }
                }
            }
            Ok(Component {
                width,
                height,
                lookup,
                faces,
                diagonal,
                neighbors,
            })
        };
        let (u, v) = crate::threading::install(|| rayon::join(|| build(0), || build(1)));
        let components = [u?, v?];
        let geometry = Geometry {
            n,
            h,
            origin,
            pressure_cells: cells.clone(),
            u_positions: components[0].faces.iter().map(|f| f.position).collect(),
            v_positions: components[1].faces.iter().map(|f| f.position).collect(),
            openings,
        };
        let problem = Arc::new(Self::build_problem(
            &components,
            cells.len(),
            h,
            viscosity,
            geometry,
        )?);
        let factor = factory.prepare(problem)?;
        Ok(Self {
            n,
            h,
            origin,
            cells,
            components,
            factor,
        })
    }

    pub fn velocity_count(&self) -> usize {
        self.components.iter().map(|c| c.faces.len()).sum()
    }

    pub fn component_split(&self) -> usize {
        self.components[0].faces.len()
    }

    fn divergence(&self, velocity: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.cells.len()];
        let mut offset = 0;
        for c in &self.components {
            for (k, f) in c.faces.iter().enumerate() {
                let flux = velocity[offset + k] / self.h;
                if f.minus != NONE {
                    result[f.minus] += flux;
                }
                if f.plus != NONE {
                    result[f.plus] -= flux;
                }
            }
            offset += c.faces.len();
        }
        result
    }

    fn transpose(&self, pressure: &[f64]) -> Vec<f64> {
        self.components
            .iter()
            .flat_map(|c| c.faces.iter())
            .map(|f| {
                (if f.minus == NONE {
                    0.0
                } else {
                    pressure[f.minus]
                } - if f.plus == NONE {
                    0.0
                } else {
                    pressure[f.plus]
                }) / self.h
            })
            .collect()
    }

    pub fn solve(
        &mut self,
        force: &[f64],
        boundary: &impl Fn([f64; 2]) -> f64,
    ) -> Result<Field, String> {
        assert_eq!(force.len(), self.velocity_count());
        let mut rhs = force.to_vec();
        for (k, f) in self
            .components
            .iter()
            .flat_map(|c| c.faces.iter())
            .enumerate()
        {
            if f.minus == NONE || f.plus == NONE {
                let p = boundary(f.position);
                rhs[k] += if f.minus == NONE {
                    p / self.h
                } else {
                    -p / self.h
                };
            }
        }
        let mut full_rhs = rhs.clone();
        full_rhs.resize(self.velocity_count() + self.cells.len(), 0.);
        let mut velocity = vec![0.; full_rhs.len()];
        let report = self.factor.solve(&full_rhs, &mut velocity)?;
        if !report.converged() {
            return Err(format!(
                "fluid solver {} failed: {}",
                report.backend, report.status
            ));
        }
        let pressure = velocity.split_off(self.velocity_count());
        let (divergence_rms, momentum_rms, net_flux) = self.measure(&velocity, &pressure, &rhs);
        Ok(Field {
            divergence_rms,
            momentum_rms,
            net_flux,
            velocity,
            pressure,
            iterations: report.iterations,
        })
    }

    pub fn boundary_rhs(&self, boundary: &impl Fn([f64; 2]) -> f64) -> Vec<f64> {
        self.components
            .iter()
            .flat_map(|c| c.faces.iter())
            .map(|f| {
                if f.minus == NONE {
                    boundary(f.position) / self.h
                } else if f.plus == NONE {
                    -boundary(f.position) / self.h
                } else {
                    0.0
                }
            })
            .collect()
    }

    pub fn measure(&self, velocity: &[f64], pressure: &[f64], rhs: &[f64]) -> (f64, f64, f64) {
        let divergence = self.divergence(velocity);
        let tp = self.transpose(pressure);
        let mut squared = 0.0;
        let mut offset = 0;
        for c in &self.components {
            for i in 0..c.faces.len() {
                let au = c.diagonal[i] * velocity[offset + i]
                    + c.neighbors[i]
                        .iter()
                        .map(|&(j, a)| a * velocity[offset + j])
                        .sum::<f64>();
                squared += (au - tp[offset + i] - rhs[offset + i]).powi(2);
            }
            offset += c.faces.len();
        }
        (
            (dot(&divergence, &divergence) / divergence.len() as f64).sqrt(),
            (squared / velocity.len() as f64).sqrt(),
            divergence.iter().sum::<f64>() * self.h * self.h,
        )
    }

    pub fn weights(&self, position: [f64; 2], axis: usize) -> Vec<(usize, f64)> {
        let c = &self.components[axis];
        let offset = if axis == 0 {
            0
        } else {
            self.components[0].faces.len()
        };
        let x = (position[0] - self.origin[0]) / self.h - if axis == 0 { 0.0 } else { 0.5 };
        let y = (position[1] - self.origin[1]) / self.h - if axis == 1 { 0.0 } else { 0.5 };
        let mut result = Vec::with_capacity(16);
        let mut total = 0.0;
        for j in y.floor() as isize - 1..=y.floor() as isize + 2 {
            for i in x.floor() as isize - 1..=x.floor() as isize + 2 {
                if i < 0 || j < 0 || i >= c.width as isize || j >= c.height as isize {
                    continue;
                }
                let k = c.lookup[j as usize * c.width + i as usize];
                if k == NONE {
                    continue;
                }
                let weight = peskin(x - i as f64) * peskin(y - j as f64);
                total += weight;
                result.push((offset + k, weight));
            }
        }
        if total > 0.0 {
            for (_, w) in &mut result {
                *w /= total;
            }
        }
        result
    }

    pub fn sample(&self, field: &Field, position: [f64; 2]) -> [f64; 2] {
        std::array::from_fn(|axis| {
            self.weights(position, axis)
                .iter()
                .map(|&(k, w)| field.velocity[k] * w)
                .sum()
        })
    }
}
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
fn peskin(r: f64) -> f64 {
    let r = r.abs();
    if r < 1.0 {
        (3.0 - 2.0 * r + (1.0 + 4.0 * r - 4.0 * r * r).sqrt()) / 8.0
    } else if r < 2.0 {
        (5.0 - 2.0 * r - (-7.0 + 12.0 * r - 4.0 * r * r).sqrt()) / 8.0
    } else {
        0.0
    }
}

impl Grid {
    fn build_problem(
        components: &[Component; 2],
        pressure_count: usize,
        h: f64,
        viscosity: f64,
        geometry: Geometry,
    ) -> Result<StokesProblem, String> {
        let nv: usize = components.iter().map(|c| c.faces.len()).sum();
        let mut entries = Vec::new();
        let mut offset = 0;
        for c in components {
            for (i, f) in c.faces.iter().enumerate() {
                let row = offset + i;
                entries.push((row, row, c.diagonal[i]));
                for &(j, a) in &c.neighbors[i] {
                    entries.push((row, offset + j, a));
                }
                // K = [ A, -B^T ; -B, 0 ], rhs = [ force ; 0 ].
                // Keep the same one-sided opening gradient as Grid::transpose.
                for (p, minus_b) in [(f.minus, -1.0 / h), (f.plus, 1.0 / h)] {
                    if p != NONE {
                        entries.push((row, nv + p, minus_b));
                        entries.push((nv + p, row, minus_b));
                    }
                }
            }
            offset += c.faces.len();
        }
        Ok(StokesProblem {
            matrix: CsrMatrix::from_triplets(nv + pressure_count, &entries)?,
            velocity_count: nv,
            viscosity,
            geometry: Some(geometry),
        })
    }
}
