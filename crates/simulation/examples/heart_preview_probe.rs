//! Actual bounded MoveInput traversal; no teleporting or synthetic geometry proof.
use rapier2d::prelude::Vector;
use simulation::{
    Config, MoveInput, Simulation,
    circulation::graph::GraphNetwork,
    map::{MapDefinition, heart_preview_route},
};
use std::{fmt::Write, path::Path};

#[path = "support/map_svg.rs"]
mod map_svg;

// These source-backed drawings are diagnostics, never velocity fields or game screenshots.
fn export_diagnostics(map: &MapDefinition, directory: &Path) {
    std::fs::create_dir_all(directory).unwrap();
    let viscosity = Config::default().viscosity;
    let network = GraphNetwork::new(map, viscosity);
    let unit = network.solve(1.);
    let pressure = 48. / unit.rates[map.pump_edge];
    let flow = network.solve(pressure);
    println!(
        "graph_viscosity={viscosity},unit_rate={},reference_pressure={pressure},reference_rate={}",
        unit.rates[map.pump_edge], flow.rates[map.pump_edge]
    );
    let mut csv = String::from(
        "edge,from,to,diameter,length,rate,planar_mean_q_over_d,pressure_from,pressure_to,viscosity,reference_drive\n",
    );
    for (i, edge) in map.edges.iter().enumerate() {
        writeln!(
            csv,
            "{i},{},{},{},{},{},{},{},{},{viscosity},{pressure}",
            edge.from,
            edge.to,
            edge.half_width * 2.,
            map.edge_length(i),
            flow.rates[i],
            flow.rates[i] / (edge.half_width * 2.),
            flow.pressures[edge.from],
            flow.pressures[edge.to]
        )
        .unwrap();
    }
    std::fs::write(directory.join("actual-graph-flow.csv"), csv).unwrap();
    for (name, bounds) in [
        ("overview", [-740., -1410., 1480., 2860.]),
        ("heart-ports", [-48., -150., 288., 390.]),
        ("lung-micro-detail", [-140., -1330., 280., 150.]),
    ] {
        let [x, y, w, h] = bounds;
        let mut svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{x} {y} {w} {h}"><title>Actual heart map diagnostic: {name}; zero runtime flow, not a game screenshot</title><rect x="{x}" y="{y}" width="{w}" height="{h}" fill="#172333"/>"##
        );
        map_svg::write_geometry(&mut svg, map, |i| {
            format!(
                "edge {i}, D{}, Q{}",
                map.edges[i].half_width * 2.,
                flow.rates[i]
            )
        });
        if name == "heart-ports" {
            // Dashed square marks the proposed baking boundary only, not flowing geometry.
            svg.push_str(r##"<rect x="-32" y="-32" width="256" height="256" fill="none" stroke="#ffffff" stroke-width="0.8" stroke-dasharray="4 3"/>"##);
            for (x, label) in [
                (38, "D10 in"),
                (78, "D9 out"),
                (112, "D10 in"),
                (150, "D10 out"),
            ] {
                write!(svg, r##"<circle cx="{x}" cy="-32" r="2" fill="white"/><text x="{x}" y="-40" font-size="5" fill="white" text-anchor="middle">{label}</text>"##).unwrap();
            }
        }
        let font: f32 = w / 45.;
        write!(svg, r##"<text x="{}" y="{}" font-size="{font}" fill="white">DIAGNOSTIC: {name}; zero runtime flow</text></svg>"##, x + w * 0.02, y + (h * 0.025).max(font * 1.3)).unwrap();
        std::fs::write(directory.join(format!("{name}.svg")), svg).unwrap();
    }
}

fn main() {
    let mut sim = Simulation::new_heart_preview(Config {
        robot_speed: 8.,
        ..Default::default()
    });
    if let Some(directory) = std::env::args().nth(1) {
        export_diagnostics(sim.world_map().unwrap(), Path::new(&directory));
        if std::env::args().nth(2).as_deref() == Some("--diagnostics-only") {
            return;
        }
    }
    let mut ticks = 0_u64;
    let mut actual_distance = 0.;
    let mut peak = 0;
    let mut min_clearance = f32::INFINITY;
    let mut min_pose = [0.; 2];
    let mut min_segment = "";
    let mut entries = [0.; 2];
    println!("waypoint,x,y,wall_distance,loaded_chunks");
    for (name, target) in heart_preview_route() {
        let mut reached = false;
        for _ in 0..2400 {
            let state = sim.snapshot();
            let d = [target[0] - state.robot[0], target[1] - state.robot[1]];
            if d[0].hypot(d[1]) < 0.25 {
                reached = true;
                break;
            }
            sim.tick(MoveInput {
                x: d[0] / d[0].hypot(d[1]) * (d[0].hypot(d[1]) * 2.).min(1.),
                y: d[1] / d[0].hypot(d[1]) * (d[0].hypot(d[1]) * 2.).min(1.),
            });
            let previous = state.robot;
            ticks += 1;
            let state = sim.snapshot();
            actual_distance += (state.robot[0] - previous[0]).hypot(state.robot[1] - previous[1]);
            let distance = sim
                .world_map()
                .unwrap()
                .signed_distance(Vector::new(state.robot[0], state.robot[1]));
            if -distance < min_clearance {
                min_clearance = -distance;
                min_pose = state.robot;
                min_segment = name;
            }
            assert!(
                -distance >= 1.0,
                "insufficient conservative route clearance: {}, segment={name}, pose={:?}, target={target:?}, global_min={min_clearance}",
                -distance,
                state.robot
            );
            assert!(state.cells.is_empty());
            assert_eq!(state.immunity.white_bodies, 0);
            assert!(state.branch_flows.iter().all(|v| *v == 0.));
            assert_eq!(state.flow.rate, 0.);
            assert_eq!(state.flow.pressure_drop, 0.);
            peak = peak.max(state.streaming.unwrap().loaded_chunks);
        }
        let s = sim.snapshot();
        assert!(reached, "failed {name}: {:?}, target {target:?}", s.robot);
        let wall = sim
            .world_map()
            .unwrap()
            .signed_distance(Vector::new(s.robot[0], s.robot[1]));
        println!(
            "{name},{:.3},{:.3},{wall:.3},{}",
            s.robot[0],
            s.robot[1],
            s.streaming.unwrap().loaded_chunks
        );
        match name {
            "right_entry" => entries[0] = s.robot[1],
            "left_entry" => entries[1] = s.robot[1],
            "right_deep" => {
                let span = s.robot[1] - entries[0];
                assert!(span >= 80.);
                println!("right_actual_span={span:.3}");
            }
            "left_deep" => {
                let span = s.robot[1] - entries[1];
                assert!(span >= 80.);
                println!("left_actual_span={span:.3}");
            }
            _ => {}
        }
    }
    println!(
        "ticks={ticks},actual_distance={actual_distance:.3},waypoints={},min_pose={min_pose:?},segment={min_segment}",
        heart_preview_route().len()
    );
    assert!(
        min_clearance >= 1.0,
        "insufficient conservative route clearance: {min_clearance}"
    );
    assert_eq!(peak, 25);
    println!(
        "peak_chunks={peak},minimum_actual_clearance={min_clearance:.3},zero_cells=true,zero_pressure=true"
    );
}
