#![allow(dead_code)]
mod accounting;
mod lbm;
mod reference_lbm;
use accounting::*;
use blood_fluid_gpu::lbm as prod;
use lbm::*;
use simulation::{
    circulation::{Flow, graph::GraphFlow},
    lbm_geometry::{opening_profiles, rasterize},
    map::{MapDefinition, VesselEdge},
};
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};
fn main() {
    run().unwrap();
}
fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let get = |key: &str, default: &str| {
        args.iter()
            .position(|a| a == key)
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| default.into())
    };
    let frames: usize = get("--frames", "500")
        .parse()
        .map_err(|_| "invalid frames")?;
    let control_frames: usize = get("--prefix-control", "3")
        .parse()
        .map_err(|_| "invalid prefix-control")?;
    let allow = args.iter().any(|a| a == "--allow-software");
    let out = PathBuf::from(get("--output", "results/hardware"));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let nodes = vec![[-50., 24.], [24., 24.], [100., 62.], [-14., 100.]];
    let pairs = [(0, 1), (1, 2), (1, 3)];
    let edges = pairs
        .into_iter()
        .map(|(from, to)| VesselEdge {
            from,
            to,
            half_width: 3.,
            centerline: vec![nodes[from], nodes[to]],
        })
        .collect();
    let map = MapDefinition {
        nodes,
        edges,
        pump_edge: 0,
        spawn: [24., 24.],
        landmarks: vec![],
    };
    let flow = GraphFlow {
        pressures: vec![],
        rates: vec![18.529297, 18.529297 * 0.7, 18.529297 * 0.3],
        summary: Flow {
            pressure_drop: 0.,
            resistance: 0.,
            rate: 18.529297,
            mean_speed: 0.,
        },
    };
    let profile = GridProfile::Fine;
    let h = profile.cell_spacing();
    let dt = (1_f32 / 120.) / 7.;
    let n = profile.cells_per_side() as usize;
    let count = n * n;
    let spec = WindowSpec {
        origin: WindowOrigin {
            block_x: 0,
            block_y: 0,
        },
        profile,
    };
    let pspec = prod::WindowSpec {
        origin: prod::WindowOrigin {
            block_x: 0,
            block_y: 0,
        },
        profile: prod::GridProfile::Fine,
    };
    let raster = rasterize(&map, &flow, pspec)?;
    let openings: Vec<_> = raster
        .geometry
        .openings
        .iter()
        .map(|o| OpeningSpec {
            side: match o.side {
                prod::WindowSide::Left => WindowSide::Left,
                prod::WindowSide::Right => WindowSide::Right,
                prod::WindowSide::Bottom => WindowSide::Bottom,
                prod::WindowSide::Top => WindowSide::Top,
            },
            cells: o.cells.clone(),
            direction: o.direction,
            outward_flow: o.outward_flow,
            background_speed: o.background_speed,
        })
        .collect();
    let geometry = WindowGeometry {
        solid: raster.geometry.solid.clone(),
        openings,
    };
    let mut w = LbmWindow::new(
        spec,
        WindowConfig {
            dt,
            viscosity: 0.05,
            allow_software: allow,
        },
        InitialField {
            density: raster.initial.density.clone(),
            velocity: raster.initial.velocity.clone(),
        },
        geometry.clone(),
    )?;
    let mut control = if control_frames > 0 {
        Some(reference_lbm::LbmWindow::new(
            reference_lbm::WindowSpec {
                origin: reference_lbm::WindowOrigin {
                    block_x: 0,
                    block_y: 0,
                },
                profile: reference_lbm::GridProfile::Fine,
            },
            reference_lbm::WindowConfig {
                dt,
                viscosity: 0.05,
                allow_software: allow,
            },
            reference_lbm::InitialField {
                density: raster.initial.density.clone(),
                velocity: raster.initial.velocity.clone(),
            },
            reference_lbm::WindowGeometry {
                solid: geometry.solid.clone(),
                openings: geometry
                    .openings
                    .iter()
                    .map(|o| reference_lbm::OpeningSpec {
                        side: match o.side {
                            WindowSide::Left => reference_lbm::WindowSide::Left,
                            WindowSide::Right => reference_lbm::WindowSide::Right,
                            WindowSide::Bottom => reference_lbm::WindowSide::Bottom,
                            WindowSide::Top => reference_lbm::WindowSide::Top,
                        },
                        cells: o.cells.clone(),
                        direction: o.direction,
                        outward_flow: o.outward_flow,
                        background_speed: o.background_speed,
                    })
                    .collect(),
            },
        )?)
    } else {
        None
    };
    let info = w.snapshot_diagnostics()?;
    let ref_info = control
        .as_ref()
        .map(|v| v.snapshot_diagnostics())
        .transpose()?;
    println!(
        "case=fork_a profile=Fine frames={frames} n={n} h={h} dt={dt} nu=0.05 tau={} D=1 adapter={:?} software={} reference_adapter={:?}",
        0.5 + 3. * 0.05 * dt / (h * h),
        info.adapter_name,
        info.software_adapter,
        ref_info.as_ref().map(|v| &v.adapter_name)
    );
    if let Some(r) = ref_info {
        if r.adapter_name != info.adapter_name || r.software_adapter != info.software_adapter {
            return Err("reference must use the same adapter".into());
        }
    }
    let initial = w.diagnostic_populations();
    let initial_s = totals(&initial, &geometry.solid).shifted;
    let fluid = geometry.solid.iter().filter(|s| !**s).count() as f64;
    let initial_mass = fluid + initial_s;
    println!(
        "fluid_cells={fluid} initial_shifted_sum={initial_s:.17e} initial_mass={initial_mass:.17e}"
    );
    let make = |name: &str| -> Result<BufWriter<File>, String> {
        Ok(BufWriter::new(
            File::create(out.join(name)).map_err(|e| e.to_string())?,
        ))
    };
    let mut steps = make("steps.csv")?;
    let mut ops = make("openings.csv")?;
    let mut frames_out = make("frames.csv")?;
    let mut controls = make("control.csv")?;
    writeln!(steps,"step,frame,substep,m0_shift,m1_shift,m2_shift,delta_bulk,delta_reconstruct,delta_incoming,delta_known_out,delta_tangent,link_net_in,jn2,b1_minus_c1,compatibility,global_step_residual,reconstruct_partition_residual,link_residual,chain_identity_residual,placeholder_max_abs,placeholder_mismatches,macro_trapezoid_delta,cum_bulk,cum_reconstruct,cum_total_change,cum_budget_residual,cum_macro_trapezoid").unwrap();
    writeln!(ops,"step,frame,substep,opening,target_q,q0,q2,j0,j2,delta_incoming,delta_known_out,delta_tangent,link_in,link_out,link_net_in,jn2,b1,c1,b1_minus_c1,compatibility,placeholder_max_abs,placeholder_mismatches").unwrap();
    writeln!(frames_out,"frame,steps,mass_change,mass_rel,rho_min,rho_max,max_lattice,q_error,j_sum_out,cum_bulk,cum_reconstruct,cum_incoming,cum_known_out,cum_tangent,cum_link_net_in,cum_chain,cum_compatibility,cum_jn2,cum_macro_trapezoid,cum_budget_residual,max_partition_residual,max_link_residual,max_chain_residual,placeholder_mismatches").unwrap();
    writeln!(controls,"frame,same_adapter,bitwise_equal,differing_populations,max_abs_difference,instrumented_fnv64,reference_fnv64").unwrap();
    let (mut cb, mut cr, mut ci, mut ck, mut ct, mut ce, mut cc, mut cp, mut cj, mut cm) =
        (0., 0., 0., 0., 0., 0., 0., 0., 0., 0.);
    let (mut max_partition, mut max_link, mut max_chain) = (0_f64, 0_f64, 0_f64);
    let mut mismatches = 0usize;
    for frame in 0..frames {
        let mut next = flow.clone();
        let pulse = 1. + 0.15 * (std::f32::consts::TAU * 1.2 * frame as f32 / 60.).sin();
        for q in &mut next.rates {
            *q *= pulse;
        }
        let (targets, profiles) = opening_profiles(&map, &next, &raster, pspec)?;
        w.set_opening_profiles(&targets, &profiles)?;
        if let Some(r) = &mut control {
            r.set_opening_profiles(&targets, &profiles)?;
        }
        for substep in 0..14 {
            let raw = w.advance_budget(StepInput {
                acceleration: [0.; 2],
            })?;
            if let Some(r) = &mut control {
                r.advance(reference_lbm::StepInput {
                    acceleration: [0.; 2],
                })?;
            }
            let a = totals(&raw[..9 * count], &geometry.solid);
            let b = totals(&raw[9 * count..18 * count], &geometry.solid);
            let z = totals(&raw[18 * count..27 * count], &geometry.solid);
            if z.lo <= 0.
                || !z.shifted.is_finite()
                || !z.lo.is_finite()
                || !z.hi.is_finite()
                || !z.max_lattice.is_finite()
            {
                return Err(format!("BAD frame={frame} substep={substep}"));
            }
            let budgets = opening_budgets(&raw, &geometry, f64::from(h), f64::from(dt));
            let (db, dr) = (b.shifted - a.shifted, z.shifted - b.shifted);
            let (mut di, mut dk, mut d_t, mut e, mut chain, mut compat, mut jn, mut jm0, mut jm2) =
                (0., 0., 0., 0., 0., 0., 0., 0., 0.);
            let mut ph = 0_f64;
            let mut mismatch = 0;
            let mut qerr = 0_f64;
            let step = frame * 14 + substep + 1;
            for (o, v) in budgets.iter().enumerate() {
                let en = v.lin - v.lout;
                let ch = v.b1 - v.c1;
                let co = v.tangent_delta + 2. * v.known_delta;
                di += v.incoming_delta;
                dk += v.known_delta;
                d_t += v.tangent_delta;
                e += en;
                chain += ch;
                compat += co;
                jn += v.jn2;
                jm0 += v.j0;
                jm2 += v.j2;
                ph = ph.max(v.placeholder_max);
                mismatch += v.placeholder_mismatches;
                qerr = qerr.max((v.q2 - f64::from(targets[o])).abs());
                writeln!(ops,"{step},{frame},{substep},{o},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{}",targets[o],v.q0,v.q2,v.j0,v.j2,v.incoming_delta,v.known_delta,v.tangent_delta,v.lin,v.lout,en,v.jn2,v.b1,v.c1,ch,co,v.placeholder_max,v.placeholder_mismatches).unwrap();
            }
            let macro_delta = -(jm0 + jm2) * 0.5 * f64::from(dt) / (f64::from(h).powi(2));
            let pr = dr - di - dk - d_t;
            let lr = di - e;
            let chr = dr - jn - chain - compat;
            cb += db;
            cr += dr;
            ci += di;
            ck += dk;
            ct += d_t;
            ce += e;
            cc += chain;
            cp += compat;
            cj += jn;
            cm += macro_delta;
            mismatches += mismatch;
            max_partition = max_partition.max(pr.abs());
            max_link = max_link.max(lr.abs());
            max_chain = max_chain.max(chr.abs());
            let change = z.shifted - initial_s;
            let residual = change - cb - ci - ck - ct;
            writeln!(steps,"{step},{frame},{substep},{:.17e},{:.17e},{:.17e},{db:.17e},{dr:.17e},{di:.17e},{dk:.17e},{d_t:.17e},{e:.17e},{jn:.17e},{chain:.17e},{compat:.17e},{:.17e},{pr:.17e},{lr:.17e},{chr:.17e},{ph:.17e},{mismatch},{macro_delta:.17e},{cb:.17e},{cr:.17e},{change:.17e},{residual:.17e},{cm:.17e}",a.shifted,b.shifted,z.shifted,z.shifted-a.shifted-db-dr).unwrap();
            if substep == 13 {
                writeln!(frames_out,"{frame},{step},{change:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{qerr:.17e},{jm2:.17e},{cb:.17e},{cr:.17e},{ci:.17e},{ck:.17e},{ct:.17e},{ce:.17e},{cc:.17e},{cp:.17e},{cj:.17e},{cm:.17e},{residual:.17e},{max_partition:.17e},{max_link:.17e},{max_chain:.17e},{mismatches}",change/initial_mass,z.lo,z.hi,z.max_lattice).unwrap();
                if let Some(r) = &control {
                    let reference = r.diagnostic_populations();
                    let observed = &raw[18 * count..27 * count];
                    let differences = observed
                        .iter()
                        .zip(&reference)
                        .filter(|(a, b)| a.to_bits() != b.to_bits())
                        .count();
                    let max = observed
                        .iter()
                        .zip(&reference)
                        .map(|(a, b)| (f64::from(*a) - f64::from(*b)).abs())
                        .fold(0_f64, f64::max);
                    writeln!(
                        controls,
                        "{frame},true,{},{differences},{max:.17e},{:016x},{:016x}",
                        differences == 0,
                        checksum(observed),
                        checksum(&reference)
                    )
                    .unwrap();
                    println!(
                        "control frame={frame} bitwise_equal={} differing={differences} max_abs={max:.3e}",
                        differences == 0
                    );
                }
                if frame % 50 == 49 || frame + 1 == frames {
                    println!(
                        "frame={frame} mass_rel={:.10e} cumulative_bulk={cb:.6e} reconstruct={cr:.6e} jn2={cj:.6e} chain={cc:.6e} compatibility={cp:.6e} closure={residual:.3e}",
                        change / initial_mass
                    );
                }
            }
        }
        if frame + 1 >= control_frames {
            control = None;
        }
    }
    for f in [&mut steps, &mut ops, &mut frames_out, &mut controls] {
        f.flush().map_err(|e| e.to_string())?;
    }
    println!("COMPLETE");
    Ok(())
}
