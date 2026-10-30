//! Explicit fixed-domain functional example; never used by the default game.
#[cfg(not(feature = "lbm-gpu"))]
fn main() {
    eprintln!("offline_lbm_bake requires --features lbm-gpu");
    std::process::exit(1);
}

#[cfg(feature = "lbm-gpu")]
fn main() {
    if let Err(error) = run() {
        eprintln!("offline bake failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(feature = "lbm-gpu")]
fn run() -> Result<(), String> {
    use blood_fluid_gpu::lbm::{
        DomainSpec, WindowConfig,
        bake::{BakeSchedule, bake_mean},
    };
    use simulation::{
        Config, Simulation,
        circulation::graph::GraphNetwork,
        lbm_geometry::rasterize_domain,
        map::{
            ChunkId, MapDefinition, VesselEdge,
            fluid_asset::{read_field, write_field},
            fluid_policy::{
                BackgroundSource, ChunkFluidPolicy, FieldId, FieldPort, MapFluidDefinition,
            },
        },
        offline_lbm::{resample_precomputed_field, to_precomputed_field},
    };
    use std::{
        collections::BTreeMap,
        fs::{self, File},
        io::Write,
        path::PathBuf,
    };

    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut output = None;
    let mut software = false;
    let mut functional = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output" if output.is_none() => {
                i += 1;
                output = Some(PathBuf::from(
                    args.get(i).ok_or("--output requires a directory")?,
                ));
            }
            "--allow-software" if !software => software = true,
            "--functional" if !functional => functional = true,
            other => return Err(format!("unknown or duplicate argument: {other}")),
        }
        i += 1;
    }
    if software != functional {
        return Err("software requires paired --allow-software --functional".into());
    }
    let output = output.ok_or("required: --output DIR [--allow-software --functional]")?;
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let mut report = File::create(output.join("report.txt")).map_err(|e| e.to_string())?;
    // Every graph vertex and the full return path are outside [-16,64]^2.
    let nodes = vec![[-48., 24.], [96., 24.], [96., 96.], [-48., 96.]];
    let edges = (0..4)
        .map(|from| {
            let to = (from + 1) % 4;
            VesselEdge {
                from,
                to,
                half_width: 3.,
                centerline: vec![nodes[from], nodes[to]],
            }
        })
        .collect();
    let mut map = MapDefinition {
        fluid: None,
        nodes,
        edges,
        pump_edge: 0,
        spawn: [8., 24.],
        landmarks: vec![],
    };
    let viscosity = 0.05;
    let network = GraphNetwork::new(&map, viscosity);
    let reference_pressure = 8. / network.solve(1.).rates[0];
    let flow = network.solve(reference_pressure);
    let domain = DomainSpec {
        origin: [-16.; 2],
        cells_per_side: 160,
        spacing: 0.5,
    };
    let raster = rasterize_domain(&map, &flow, domain)?;
    let config = WindowConfig {
        dt: 0.01,
        viscosity,
        allow_software: software,
    };
    let schedule = BakeSchedule {
        steps_per_frame: 4,
        warmup_frames: 300,
        sample_frames: 600,
        sample_stride: 10,
    };
    writeln!(report, "fixed straight-pipe pipeline only; no convergence or physical accuracy claim\nfunctional={functional}\ndomain={domain:?}\nsolver={config:?}\nschedule={schedule:?}\nreference_pressure={reference_pressure}\ngraph_rates={:?}\npeak_lattice_speed={}\noriginal_openings={:?}", flow.rates, 3. * flow.rates[0] / 12. * config.dt / domain.spacing, raster.geometry.openings).map_err(|e|e.to_string())?;
    report.flush().map_err(|e| e.to_string())?;
    let baked = bake_mean(domain, config, raster.initial, raster.geometry, schedule)?;
    writeln!(report, "bake_report={:#?}", baked.report).map_err(|e| e.to_string())?;
    let ports = [(48., -1.), (80., 1.)]
        .into_iter()
        .map(|(along, outward_sign)| FieldPort {
            edge: 0,
            along,
            outward_sign,
            reference_flux: flow.rates[0] * outward_sign,
        })
        .collect();
    let field = to_precomputed_field(&baked, 1, reference_pressure, 2., ports)?;
    let raw_asset = output.join("raw-cell-centered.bfld");
    write_field(File::create(&raw_asset).map_err(|e| e.to_string())?, &field)?;
    let field = resample_precomputed_field(
        &map,
        &field,
        field.origin.map(|v| v + domain.spacing * 0.5),
        domain.spacing,
        [domain.cells_per_side as usize - 1; 2],
    )?;
    writeln!(report, "raw_asset={}\nexport=production bilinear resampling; same spacing; map mask; wall/solid zero; no normalization or analytic substitution", raw_asset.display()).map_err(|e|e.to_string())?;
    let asset = output.join("straight-pipe.bfld");
    write_field(File::create(&asset).map_err(|e| e.to_string())?, &field)?;
    let loaded = read_field(File::open(&asset).map_err(|e| e.to_string())?)?;
    if loaded.velocity != field.velocity || loaded.fluid != field.fluid {
        return Err("asset roundtrip changed velocity or mask".into());
    }
    writeln!(
        report,
        "asset={}\nloaded_origin={:?}\nloaded_size={:?}\nroundtrip_velocity_mask=exact",
        asset.display(),
        loaded.origin,
        loaded.size
    )
    .map_err(|e| e.to_string())?;
    map.fluid = Some(MapFluidDefinition {
        geometry_revision: 1,
        max_base_pressure: reference_pressure,
        chunks: BTreeMap::from([0, 1].map(|x| {
            (
                ChunkId { x, y: 1 },
                ChunkFluidPolicy {
                    background: BackgroundSource::Precomputed(FieldId(0)),
                    allow_lbm: false,
                },
            )
        })),
        fields: vec![loaded],
    });
    let result = (|| -> Result<(), String> {
        let sim = Simulation::try_new_policy_world(
            Config {
                pressure: reference_pressure,
                viscosity,
                ..Config::default()
            },
            map,
        )?;
        for point in [
            [-1., 24.],
            [0., 24.],
            [8., 24.],
            [31.9, 24.],
            [32., 24.],
            [8., 21.],
        ] {
            let sample = sim.sample_world(point).ok_or("missing background sample")?;
            writeln!(report, "sample {point:?}={sample:?}").map_err(|e| e.to_string())?;
            if sample.background_velocity.iter().any(|v| !v.is_finite()) {
                return Err("nonfinite imported background".into());
            }
            if sample.local_weight != 0.
                || (point[1] == 24. && sample.background_velocity[0] <= 0.)
                || (point[1] == 21. && sample.background_velocity != [0.; 2])
            {
                return Err("unexpected imported background or local overlay".into());
            }
        }
        Ok(())
    })();
    writeln!(report, "normal_policy_import={result:?}").map_err(|e| e.to_string())?;
    report.flush().map_err(|e| e.to_string())?;
    result?;
    println!(
        "Wrote and validated {} ({} samples; bounded functional run)",
        asset.display(),
        baked.report.samples
    );
    Ok(())
}
