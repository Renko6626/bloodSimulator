use std::{
    env,
    f64::consts::PI,
    fs::File,
    io::{BufWriter, Write},
    time::Instant,
};

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

struct Config {
    n: usize,
    scene: String,
    amplitude: f64,
    tau: f64,
    max_steps: usize,
    window_time: f64,
    threshold: f64,
    field: Option<String>,
    gamma: f64,
    marker_speed: f64,
    post_steps: usize,
    post_repeats: usize,
    post_samples: Option<String>,
}

fn config() -> Result<Config, String> {
    let mut c = Config {
        n: 32,
        scene: "straight".into(),
        amplitude: 0.1,
        tau: 1.0,
        max_steps: 200_000,
        window_time: 0.01,
        threshold: 1e-5,
        field: None,
        gamma: 0.25,
        marker_speed: 0.1,
        post_steps: 0,
        post_repeats: 1,
        post_samples: None,
    };
    let mut force_provided = false;
    let mut args = env::args().skip(1);
    while let Some(key) = args.next() {
        let v = args.next().ok_or(format!("missing value for {key}"))?;
        match key.as_str() {
            "--n" => c.n = v.parse().map_err(|_| "invalid n")?,
            "--scene" => c.scene = v,
            "--force-amplitude" => {
                c.amplitude = v.parse().map_err(|_| "invalid force amplitude")?;
                force_provided = true;
            }
            "--tau" => c.tau = v.parse().map_err(|_| "invalid tau")?,
            "--max-steps" => c.max_steps = v.parse().map_err(|_| "invalid max steps")?,
            "--window-time" => c.window_time = v.parse().map_err(|_| "invalid window time")?,
            "--threshold" => c.threshold = v.parse().map_err(|_| "invalid threshold")?,
            "--field" => c.field = Some(v),
            "--gamma" => c.gamma = v.parse().map_err(|_| "invalid gamma")?,
            "--marker-speed" => c.marker_speed = v.parse().map_err(|_| "invalid marker speed")?,
            "--post-steps" => c.post_steps = v.parse().map_err(|_| "invalid post steps")?,
            "--post-repeats" => c.post_repeats = v.parse().map_err(|_| "invalid post repeats")?,
            "--post-samples" => c.post_samples = Some(v),
            _ => return Err(format!("unknown option {key}")),
        }
    }
    if c.scene == "blob4" {
        if force_provided && c.amplitude != 0.0 {
            return Err("blob4 requires zero background force".into());
        }
        c.amplitude = 0.0;
    }
    if ![32, 64].contains(&c.n)
        || !["straight", "channel", "blob4"].contains(&c.scene.as_str())
        || !c.amplitude.is_finite()
        || c.amplitude < 0.0
        || !c.tau.is_finite()
        || c.tau <= 0.5
        || !c.gamma.is_finite()
        || c.gamma <= 0.0
        || !c.marker_speed.is_finite()
        || c.marker_speed <= 0.0
        || c.max_steps == 0
        || !c.window_time.is_finite()
        || c.window_time <= 0.0
        || !c.threshold.is_finite()
        || c.threshold <= 0.0
        || (c.scene != "blob4" && c.amplitude == 0.0)
        || c.post_repeats == 0
        || (c.post_steps == 0 && (c.post_repeats != 1 || c.post_samples.is_some()))
    {
        return Err("invalid parameters: n=32|64, scene=straight|channel|blob4, amplitude>=0 (positive for straight/channel), tau>0.5, gamma>0, marker-speed>0, max-steps>0, window-time>0, threshold>0".into());
    }
    Ok(c)
}

fn equilibrium(rho: f64, ux: f64, uy: f64, k: usize) -> f64 {
    let cu = (CX[k] as f64) * ux + (CY[k] as f64) * uy;
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

fn marker_stencils(n: usize) -> Vec<Vec<(usize, f64)>> {
    let points = [(0.425, 0.5), (0.575, 0.5), (0.5, 0.425), (0.5, 0.575)];
    points
        .iter()
        .map(|&(px, py)| {
            let mut out = Vec::new();
            for y in 0..n {
                for x in 0..n {
                    let dx = ((x as f64 + 0.5) / n as f64 - px) * n as f64;
                    let dy = ((y as f64 + 0.5) / n as f64 - py) * n as f64;
                    let wx = peskin_phi(dx);
                    let wy = peskin_phi(dy);
                    if wx * wy > 0.0 {
                        out.push((y * n + x, wx * wy));
                    }
                }
            }
            out
        })
        .collect()
}

fn stream_targets(n: usize, solid: &[bool]) -> (Vec<usize>, Vec<[(usize, usize); 9]>) {
    let mut active = Vec::new();
    let mut targets = vec![[(0, 0); 9]; n * n];
    let mut written = vec![false; n * n * 9];
    for y in 0..n {
        for x in 0..n {
            let i = y * n + x;
            if solid[i] {
                continue;
            }
            active.push(i);
            for k in 0..9 {
                let yy = y as isize + CY[k];
                let xx = (x as isize + CX[k]).rem_euclid(n as isize) as usize;
                let dst = if yy < 0 || yy >= n as isize || solid[yy as usize * n + xx] {
                    (i, OPP[k])
                } else {
                    (yy as usize * n + xx, k)
                };
                assert!(
                    !written[dst.0 * 9 + dst.1],
                    "stream destination must have one writer"
                );
                written[dst.0 * 9 + dst.1] = true;
                targets[i][k] = dst;
            }
        }
    }
    assert!(active.iter().all(|&i| (0..9).all(|k| written[i * 9 + k])));
    (active, targets)
}

fn advance(
    f: &mut Vec<[f64; 9]>,
    next: &mut Vec<[f64; 9]>,
    active: &[usize],
    targets: &[[(usize, usize); 9]],
    force: &mut [[f64; 2]],
    stencils: &[Vec<(usize, f64)>],
    marker_forces: &mut [[f64; 2]; 4],
    c: &Config,
    dx: f64,
    dt: f64,
    tau_inv: f64,
    guo_factor: f64,
) -> bool {
    let bg_lu = c.amplitude * dt * dt / dx;
    if c.scene == "blob4" {
        // Explicit lag: interpolate the previous state's velocity including its previous half force.
        for (m, stencil) in stencils.iter().enumerate() {
            let mut ju = [0.0; 2];
            for &(i, w) in stencil {
                let (_, ux, uy) = macrostate(&f[i], force[i]);
                ju[0] += w * ux * dx / dt;
                ju[1] += w * uy * dx / dt;
            }
            let r = c.gamma / 4.0;
            marker_forces[m] = [r * (c.marker_speed - ju[0]), -r * ju[1]];
        }
    }
    force.fill([bg_lu, 0.0]);
    if c.scene == "blob4" {
        for (m, stencil) in stencils.iter().enumerate() {
            for &(i, w) in stencil {
                let scale = w / (dx * dx) * dt * dt / dx; // physical point force -> density -> lattice force
                force[i][0] += scale * marker_forces[m][0];
                force[i][1] += scale * marker_forces[m][1];
            }
        }
    }
    for &i in active {
        let (rho, ux, uy) = macrostate(&f[i], force[i]);
        if !rho.is_finite() || rho <= 0.0 || !ux.is_finite() || !uy.is_finite() {
            return false;
        }
        for k in 0..9 {
            let cx = CX[k] as f64;
            let cy = CY[k] as f64;
            let cu = cx * ux + cy * uy;
            let guo = W[k]
                * guo_factor
                * ((3.0 * (cx - ux) + 9.0 * cu * cx) * force[i][0]
                    + (3.0 * (cy - uy) + 9.0 * cu * cy) * force[i][1]);
            let post = f[i][k] - (f[i][k] - equilibrium(rho, ux, uy, k)) * tau_inv + guo;
            let (dst, q) = targets[i][k];
            next[dst][q] = post;
        }
    }
    std::mem::swap(f, next);
    true
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let c = config()?;
    let init_start = Instant::now();
    let n = c.n;
    let dx = 1.0 / n as f64;
    let nu_lu = (c.tau - 0.5) / 3.0;
    let dt = nu_lu * dx * dx; // physical nu=1
    let accel_lu = c.amplitude * dt * dt / dx;
    let mut solid = vec![false; n * n];
    for y in 0..n {
        for x in 0..n {
            let xx = (x as f64 + 0.5) * dx;
            let yy = (y as f64 + 0.5) * dx;
            let bottom = 0.12 + 0.04 * (2.0 * PI * xx).sin();
            let top = 0.88 - 0.08 * (2.0 * PI * xx).sin();
            solid[y * n + x] = c.scene == "channel" && (yy < bottom || yy > top);
        }
    }
    let fluid_count = solid.iter().filter(|&&v| !v).count();
    let (active, targets) = stream_targets(n, &solid);
    let tau_inv = 1.0 / c.tau;
    let guo_factor = 1.0 - 0.5 * tau_inv;
    let mut f = vec![[0.0; 9]; n * n];
    let mut next = vec![[0.0; 9]; n * n];
    let stencils = if c.scene == "blob4" {
        marker_stencils(n)
    } else {
        Vec::new()
    };
    let mut marker_forces = [[0.0; 2]; 4];
    let mut force = vec![[0.0; 2]; n * n];
    for i in 0..n * n {
        if !solid[i] {
            for k in 0..9 {
                f[i][k] = W[k];
            }
        }
    }
    let init_ms = init_start.elapsed().as_secs_f64() * 1e3;

    let window_steps = (c.window_time / dt).round().max(1.0) as usize;
    let mut prior = vec![[0.0; 2]; n * n];
    let mut residual = f64::INFINITY;
    let mut consecutive = 0;
    let mut steps = 0;
    let mut bad = false;
    let u_scale = if c.scene == "blob4" {
        c.marker_speed
    } else {
        c.amplitude
    };
    let solve_start = Instant::now();
    while steps < c.max_steps {
        if !advance(
            &mut f,
            &mut next,
            &active,
            &targets,
            &mut force,
            &stencils,
            &mut marker_forces,
            &c,
            dx,
            dt,
            tau_inv,
            guo_factor,
        ) {
            bad = true;
            break;
        }
        steps += 1;
        if steps % window_steps == 0 {
            let mut d2 = 0.0;
            for i in 0..n * n {
                if solid[i] {
                    continue;
                }
                let (_, ux, uy) = macrostate(&f[i], force[i]);
                let vx = ux * dx / dt;
                let vy = uy * dx / dt;
                d2 += (vx - prior[i][0]).powi(2) + (vy - prior[i][1]).powi(2);
                prior[i] = [vx, vy];
            }
            residual = (d2 / fluid_count as f64).sqrt() / (window_steps as f64 * dt * u_scale);
            if steps as f64 * dt >= 1.0 && residual < c.threshold {
                consecutive += 1;
            } else {
                consecutive = 0;
            }
            if consecutive >= 3 {
                break;
            }
        }
    }
    let solve_ms = solve_start.elapsed().as_secs_f64() * 1e3;
    let converged = consecutive >= 3 && !bad;
    let mut post_ms = 0.0;
    let mut post_completed = 0;
    let mut post_samples = Vec::new();
    if converged && c.post_steps > 0 {
        for _ in 0..c.post_repeats {
            let start = Instant::now();
            for _ in 0..c.post_steps {
                if !advance(
                    &mut f,
                    &mut next,
                    &active,
                    &targets,
                    &mut force,
                    &stencils,
                    &mut marker_forces,
                    &c,
                    dx,
                    dt,
                    tau_inv,
                    guo_factor,
                ) {
                    bad = true;
                    break;
                }
                post_completed += 1;
            }
            let elapsed = start.elapsed().as_secs_f64() * 1e3;
            post_samples.push(elapsed);
            post_ms += elapsed;
            if bad {
                break;
            }
        }
    }
    if let Some(path) = &c.post_samples {
        let mut w = BufWriter::new(File::create(path)?);
        writeln!(w, "block,steps,physical_time,elapsed_ms")?;
        for (block, ms) in post_samples.iter().enumerate() {
            let block_steps = if block + 1 == post_samples.len() {
                post_completed - block * c.post_steps
            } else {
                c.post_steps
            };
            writeln!(
                w,
                "{},{},{:.12e},{:.9}",
                block + 1,
                block_steps,
                block_steps as f64 * dt,
                ms
            )?;
        }
    }
    let post_mean = if post_samples.is_empty() {
        f64::NAN
    } else {
        post_ms / post_samples.len() as f64
    };
    let post_p95 = if post_samples.is_empty() {
        f64::NAN
    } else {
        let mut sorted = post_samples.clone();
        sorted.sort_by(f64::total_cmp);
        sorted[((0.95 * sorted.len() as f64).ceil() as usize).saturating_sub(1)]
    };
    let mut density_min = f64::INFINITY;
    let mut density_max = f64::NEG_INFINITY;
    let mut mass = 0.0;
    let mut max_speed: f64 = 0.0;
    let mut vel = vec![[0.0; 2]; n * n];
    let mut rho_field = vec![0.0; n * n];
    for i in 0..n * n {
        if solid[i] {
            continue;
        }
        let (rho, ux, uy) = macrostate(&f[i], force[i]);
        density_min = density_min.min(rho);
        density_max = density_max.max(rho);
        mass += rho;
        rho_field[i] = rho;
        vel[i] = [ux * dx / dt, uy * dx / dt];
        max_speed = max_speed.max(vel[i][0].hypot(vel[i][1]));
    }
    let mut div2 = 0.0;
    let mut div_count = 0;
    for y in 1..n - 1 {
        for x in 0..n {
            let i = y * n + x;
            let xp = y * n + (x + 1) % n;
            let xm = y * n + (x + n - 1) % n;
            let yp = (y + 1) * n + x;
            let ym = (y - 1) * n + x;
            if [i, xp, xm, yp, ym].iter().any(|&j| solid[j]) {
                continue;
            }
            let div = (vel[xp][0] - vel[xm][0] + vel[yp][1] - vel[ym][1]) / (2.0 * dx);
            div2 += div * div;
            div_count += 1;
        }
    }
    let div_rms = (div2 / div_count as f64).sqrt();
    let mut body_reaction = [f64::NAN; 2];
    let mut force_balance = f64::NAN;
    let mut law_residual = f64::NAN;
    if c.scene == "blob4" {
        body_reaction = [0.0; 2];
        let mut law2 = 0.0;
        for (m, stencil) in stencils.iter().enumerate() {
            let mut ju = [0.0; 2];
            for &(i, w) in stencil {
                ju[0] += w * vel[i][0];
                ju[1] += w * vel[i][1];
            }
            let r = c.gamma / 4.0;
            law2 += (marker_forces[m][0] / r + ju[0] - c.marker_speed).powi(2)
                + (marker_forces[m][1] / r + ju[1]).powi(2);
            body_reaction[0] -= marker_forces[m][0];
            body_reaction[1] -= marker_forces[m][1];
        }
        law_residual = (law2 / 8.0).sqrt() / c.marker_speed;
        let mut fluid_force = [0.0; 2];
        for q in &force {
            fluid_force[0] += q[0] * dx.powi(3) / dt.powi(2);
            fluid_force[1] += q[1] * dx.powi(3) / dt.powi(2);
        }
        force_balance =
            (fluid_force[0] + body_reaction[0]).hypot(fluid_force[1] + body_reaction[1]);
    }
    let mut l2 = 0.0;
    let mut ref2 = 0.0;
    let mut flux = 0.0;
    let mut wall_estimate = f64::NAN;
    if c.scene == "straight" {
        for y in 0..n {
            for x in 0..n {
                let i = y * n + x;
                let yy = (y as f64 + 0.5) * dx;
                let truth = c.amplitude * yy * (1.0 - yy) / 2.0;
                l2 += (vel[i][0] - truth).powi(2) + vel[i][1].powi(2);
                ref2 += truth * truth;
            }
        }
        flux = (0..n).map(|y| vel[y * n][0] * dx).sum();
        // Quadratic extrapolation from y=h/2, 3h/2, 5h/2 to the link wall y=0.
        wall_estimate = 1.875 * vel[0][0] - 1.25 * vel[n][0] + 0.375 * vel[2 * n][0];
    }
    if let Some(path) = &c.field {
        let mut w = BufWriter::new(File::create(path)?);
        writeln!(w, "x,y,u,v,p,density,solid")?;
        for y in 0..n {
            for x in 0..n {
                let i = y * n + x;
                let p = (rho_field[i] - 1.0) * dx * dx / (3.0 * dt * dt);
                writeln!(
                    w,
                    "{:.12e},{:.12e},{:.12e},{:.12e},{:.12e},{:.12e},{}",
                    (x as f64 + 0.5) * dx,
                    (y as f64 + 0.5) * dx,
                    vel[i][0],
                    vel[i][1],
                    if solid[i] { 0.0 } else { p },
                    rho_field[i],
                    u8::from(solid[i])
                )?;
            }
        }
    }
    println!("scene,n,tau,force_amplitude,gamma,marker_speed,dx,dt,nu_lu,force_lu,fluid_cells,window_steps,steps_to_t1,settle_steps,settle_physical_time,converged,unstable,steady_r,init_ms,solve_ms,us_per_fluid_cell_step,post_steps,post_physical_time,post_ms,max_speed,re_l1,density_min,density_max,mass_relative_error,div_center_fd_rms,poiseuille_relative_l2,flux,flux_relative_error,wall_velocity_extrapolated,reaction_x,reaction_y,force_balance_abs,force_law_velocity_relative_rms,post_repeats,post_block_steps,post_ms_mean,post_ms_p95");
    println!("{},{},{:.8},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{},{},{},{},{:.8e},{},{},{:.8e},{:.6},{:.6},{:.8e},{},{:.8e},{:.6},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{:.8e},{},{},{:.6},{:.6}",
        c.scene,n,c.tau,c.amplitude,c.gamma,c.marker_speed,dx,dt,nu_lu,accel_lu,fluid_count,window_steps,(1.0/dt).ceil() as usize,steps,steps as f64*dt,
        converged,bad,residual,init_ms,solve_ms,solve_ms*1e3/(fluid_count*steps).max(1) as f64,
        post_completed,post_completed as f64*dt,post_ms,max_speed,max_speed,density_min,density_max,mass/fluid_count as f64-1.0,div_rms,
        if c.scene=="straight" {(l2/ref2).sqrt()} else {f64::NAN},flux,
        if c.scene=="straight" {flux/(c.amplitude/12.0)-1.0} else {f64::NAN},wall_estimate,
        body_reaction[0],body_reaction[1],force_balance,law_residual,post_samples.len(),c.post_steps,post_mean,post_p95);
    if !converged || bad {
        std::process::exit(2);
    }
    Ok(())
}
