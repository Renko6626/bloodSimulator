//! Explicit heart-background job. Preflight never creates a GPU or physics world.
#[cfg(not(feature = "lbm-gpu"))]
fn main() {
    eprintln!("organ_heart_bake requires --features lbm-gpu");
    std::process::exit(1);
}

#[cfg(feature = "lbm-gpu")]
fn main() {
    if let Err(error) = job::run() {
        eprintln!("heart background: {error}");
        std::process::exit(1);
    }
}

#[cfg(feature = "lbm-gpu")]
mod job {
    use blood_fluid_gpu::lbm::{
        DomainSpec, WindowConfig, WindowGeometry, WindowSide,
        bake::{BakeSchedule, bake_mean_with_profiles},
    };
    use rapier2d::prelude::Vector;
    use simulation::{
        Config, Simulation,
        circulation::graph::GraphNetwork,
        lbm_geometry::rasterize_domain,
        map::{
            MapDefinition,
            fluid_asset::{read_field, write_field},
            fluid_policy::FieldPort,
            organ_flow,
        },
        offline_lbm::{to_boundary_aligned_field, to_precomputed_field},
    };
    use std::{
        collections::{BTreeMap, VecDeque},
        fs::{self, File},
        io::Write,
        path::PathBuf,
    };

    pub fn run() -> Result<(), String> {
        let mut args = std::env::args().skip(1);
        let mut output = None;
        let mut mode = None;
        let mut software = false;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--output" if output.is_none() => {
                    output = Some(PathBuf::from(args.next().ok_or("--output needs DIR")?));
                }
                "--preflight" | "--bake" if mode.is_none() => mode = Some(arg),
                "--allow-software" if !software => software = true,
                _ => return Err(format!("unknown or duplicate argument: {arg}")),
            }
        }
        let output = output.ok_or("required: --output DIR --preflight|--bake")?;
        let mode = mode.ok_or("choose --preflight or --bake")?;
        if mode == "--preflight" && software {
            return Err("preflight is CPU-only; omit --allow-software".into());
        }
        fs::create_dir_all(&output).map_err(|e| e.to_string())?;
        let mut report = File::create(output.join(if mode == "--preflight" {
            "preflight.txt"
        } else {
            "report.txt"
        }))
        .map_err(|e| e.to_string())?;
        let result = execute(&mut report, &output, mode == "--bake", software);
        writeln!(report, "result={result:?}").map_err(|e| e.to_string())?;
        result
    }

    fn execute(
        report: &mut File,
        output: &std::path::Path,
        bake: bool,
        software: bool,
    ) -> Result<(), String> {
        let mut map = MapDefinition::organ_world();
        let network = GraphNetwork::new(&map, 0.05);
        let pressure = organ_flow::REFERENCE_RATE / network.solve(1.).rates[map.pump_edge];
        let flow = network.solve(pressure);
        let ports = organ_flow::ports(&map, &flow.rates)?;
        let domain = DomainSpec {
            origin: organ_flow::DOMAIN_ORIGIN,
            cells_per_side: organ_flow::DOMAIN_CELLS,
            spacing: organ_flow::SPACING,
        };
        let config = WindowConfig {
            dt: 0.0025,
            viscosity: 0.05,
            allow_software: software,
        };
        let schedule = BakeSchedule {
            steps_per_frame: 16,
            warmup_frames: 1500,
            sample_frames: 1500,
            sample_stride: 25,
        };
        writeln!(report, "map=production organ_world; no geometry changes\nreference_Q=48\nreference_pressure={pressure:.9}\ndomain={domain:?}\nconfig={config:?}\nschedule={schedule:?}\nwall_clock_limit_seconds=1200 (external timeout)\nlabels=[464,656]..[608,848]\nports={ports:#?}").map_err(|e| e.to_string())?;
        report.flush().map_err(|e| e.to_string())?;
        source_boundary(report, &map, &ports)?;
        let raster = rasterize_domain(&map, &flow, domain)?;
        let max_speed = raster
            .initial
            .velocity
            .iter()
            .map(|u| u[0].hypot(u[1]))
            .fold(0_f32, f32::max);
        writeln!(report, "openings={:#?}\ninitial_max_world_speed={max_speed}\ninitial_max_lattice_speed={}\nwarmup_seconds={}\nsample_seconds={}\nsamples={}", raster.openings, max_speed * config.dt / domain.spacing, schedule.warmup_frames as f32 * 16. * config.dt, schedule.sample_frames as f32 * 16. * config.dt, schedule.sample_count()?).map_err(|e| e.to_string())?;
        if max_speed * config.dt / domain.spacing > 0.05 {
            return Err("initial lattice speed exceeds 0.05 budget".into());
        }
        components(report, &raster.geometry, domain.cells_per_side as usize)?;
        report.flush().map_err(|e| e.to_string())?;
        println!(
            "CPU preflight passed; pressure={pressure:.9}; see {}",
            output.display()
        );
        if !bake {
            return Ok(());
        }
        println!(
            "bounded bake: 512², 48000 steps, 60s warmup + 60s sampling, 60 samples; 1200s external limit"
        );
        let profiles: Vec<_> = raster
            .openings
            .iter()
            .map(|o| o.normal_velocity.clone())
            .collect();
        writeln!(report, "prescribed_opening_profiles={profiles:?}").map_err(|e| e.to_string())?;
        let baked = bake_mean_with_profiles(
            domain,
            config,
            raster.initial,
            raster.geometry,
            Some(&profiles),
            schedule,
        )?;
        writeln!(report, "bake_report={:#?}", baked.report).map_err(|e| e.to_string())?;
        report.flush().map_err(|e| e.to_string())?;
        let raw = to_precomputed_field(&baked, 1, pressure, 1.15, ports.clone())?;
        write_field(
            File::create(output.join("raw-cell-centered.bfld")).map_err(|e| e.to_string())?,
            &raw,
        )?;
        let field = to_boundary_aligned_field(&map, &flow, &baked, 1, pressure, 1.15, ports)?;
        let asset = output.join("organ-heart.bfld");
        write_field(File::create(&asset).map_err(|e| e.to_string())?, &field)?;
        let loaded = read_field(File::open(asset).map_err(|e| e.to_string())?)?;
        if loaded.velocity != field.velocity || loaded.fluid != field.fluid {
            return Err("asset roundtrip changed velocity or mask".into());
        }
        map.fluid = Some(organ_flow::definition(loaded));
        let mut sim_config = Config {
            pressure,
            pulse_hz: 0.,
            cell_count: 0,
            ..Config::default()
        };
        sim_config.white_cells.count = 0;
        let imported = Simulation::try_new_policy_world(sim_config, map);
        writeln!(
            report,
            "normal_policy_import={:?}",
            imported.as_ref().map(|_| ())
        )
        .map_err(|e| e.to_string())?;
        imported?;
        println!("asset roundtrip and original policy import passed");
        Ok(())
    }

    fn source_boundary(
        report: &mut File,
        map: &MapDefinition,
        ports: &[FieldPort],
    ) -> Result<(), String> {
        let [lo, hi] = [organ_flow::SOURCE_MIN, organ_flow::SOURCE_MAX];
        for chamber in &map.chambers {
            let [min, max] = chamber.bounds();
            if min[0] < lo[0] || max[0] > hi[0] || min[1] < lo[1] || max[1] > hi[1] {
                return Err(format!(
                    "background rectangle cuts chamber {}",
                    chamber.name
                ));
            }
        }
        for port in ports {
            let radius = map.edges[port.edge].half_width;
            if port.along < 2. * radius || map.edge_length(port.edge) - port.along < 2. * radius {
                return Err(format!("source port too close to junction: {port:?}"));
            }
        }
        let mut hits = BTreeMap::new();
        for (a, b, outward) in [
            ([lo[0], lo[1]], [lo[0], hi[1]], [-1., 0.]),
            ([hi[0], lo[1]], [hi[0], hi[1]], [1., 0.]),
            ([lo[0], lo[1]], [hi[0], lo[1]], [0., -1.]),
            ([lo[0], hi[1]], [hi[0], hi[1]], [0., 1.]),
        ] {
            let a = Vector::from_array(a);
            let b = Vector::from_array(b);
            let steps = ((b - a).length() / 0.25).ceil() as usize;
            for i in 0..=steps {
                let point = a.lerp(b, i as f32 / steps as f32);
                if map.signed_distance(point) >= 0. {
                    continue;
                }
                let edge = map.nearest_edge(point);
                let projection = map.project_edge(edge, point);
                let port = ports
                    .iter()
                    .find(|p| {
                        p.edge == edge
                            && (projection.along - p.along).abs()
                                <= map.edges[edge].half_width + 0.5
                            && projection.tangent.dot(Vector::from_array(outward)) * p.outward_sign
                                > 0.1
                    })
                    .ok_or_else(|| {
                        format!("undeclared source-boundary crossing at {point:?}, edge={edge}")
                    })?;
                *hits.entry(port.edge).or_insert(0usize) += 1;
            }
        }
        if hits.len() != 4 {
            return Err(format!("expected four source ports, got {hits:?}"));
        }
        writeln!(
            report,
            "source_boundary_cpu_samples={hits:?}; step<=0.25; all chamber bounds inside"
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn components(report: &mut File, geometry: &WindowGeometry, n: usize) -> Result<(), String> {
        let mut labels = vec![usize::MAX; n * n];
        let mut sizes = Vec::new();
        for start in 0..labels.len() {
            if geometry.solid[start] || labels[start] != usize::MAX {
                continue;
            }
            let id = sizes.len();
            let mut size = 0;
            let mut queue = VecDeque::from([start]);
            labels[start] = id;
            while let Some(k) = queue.pop_front() {
                size += 1;
                let (x, y) = (k % n, k / n);
                for next in [
                    (x > 0).then(|| k - 1),
                    (x + 1 < n).then(|| k + 1),
                    (y > 0).then(|| k - n),
                    (y + 1 < n).then(|| k + n),
                ]
                .into_iter()
                .flatten()
                {
                    if !geometry.solid[next] && labels[next] == usize::MAX {
                        labels[next] = id;
                        queue.push_back(next);
                    }
                }
            }
            sizes.push(size);
        }
        let mut flux = vec![0_f64; sizes.len()];
        let mut absolute = vec![0_f64; sizes.len()];
        for (i, opening) in geometry.openings.iter().enumerate() {
            let j = opening.cells.start as usize;
            let k = match opening.side {
                WindowSide::Left => j * n,
                WindowSide::Right => j * n + n - 1,
                WindowSide::Bottom => j,
                WindowSide::Top => (n - 1) * n + j,
            };
            let id = labels[k];
            if id == usize::MAX {
                return Err("opening does not touch fluid".into());
            }
            flux[id] += f64::from(opening.outward_flow);
            absolute[id] += f64::from(opening.outward_flow.abs());
            writeln!(
                report,
                "opening_component opening={i} component={id} flux={}",
                opening.outward_flow
            )
            .map_err(|e| e.to_string())?;
        }
        for id in 0..sizes.len() {
            writeln!(
                report,
                "component={id} fluid_cells={} signed_outward_flux={} absolute_flux={}",
                sizes[id], flux[id], absolute[id]
            )
            .map_err(|e| e.to_string())?;
            if absolute[id] == 0. || flux[id].abs() > absolute[id] * 1e-5 + 1e-6 {
                return Err(format!("unbalanced or unopened fluid component {id}"));
            }
        }
        Ok(())
    }
}
