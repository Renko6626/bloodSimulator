//! Bounded source-map traversal diagnostics. No runtime routing or teleportation.
use rapier2d::prelude::Vector;
use simulation::{
    Config, MoveInput, Simulation, circulation::graph::GraphNetwork, map::MapDefinition,
};
use std::{fmt::Write, path::Path};

type Point = [f32; 2];
struct Case {
    name: &'static str,
    points: Vec<Point>,
}
fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "spawn-right-ventricle-lung-central-upper-D3",
            points: vec![
                [499., 722.],
                [499., 736.],
                [499., 760.],
                [511., 820.],
                [512., 750.],
                [519., 735.],
                [528., 715.],
                [528., 705.],
                [528., 646.],
                [528., 626.],
                [528., 550.],
                [480., 500.],
                [440., 400.],
                [450., 220.],
                [490., 240.],
                [520., 250.],
                [555., 260.],
                [590., 230.],
                [570., 170.],
                [555., 140.],
                [565., 115.],
            ],
        },
        Case {
            name: "body-A-junction",
            points: vec![
                [940., 820.],
                [900., 800.],
                [860., 760.],
                [850., 720.],
                [820., 700.],
                [740., 670.],
                [720., 760.],
                [780., 850.],
            ],
        },
        Case {
            name: "body-B-D3",
            points: vec![
                [600., 1030.],
                [550., 1000.],
                [565., 970.],
                [600., 985.],
                [650., 950.],
                [600., 910.],
            ],
        },
        Case {
            name: "body-C-junction",
            points: vec![
                [300., 450.],
                [320., 500.],
                [320., 550.],
                [330., 600.],
                [280., 670.],
                [340., 700.],
            ],
        },
    ]
}
fn clearance(map: &MapDefinition, p: Point) -> f32 {
    -map.signed_distance(Vector::new(p[0], p[1]))
}
fn export_graph(map: &MapDefinition, directory: &Path) {
    std::fs::create_dir_all(directory).unwrap();
    let viscosity = Config::default().viscosity;
    let flow = GraphNetwork::new(map, viscosity).solve(1.);
    let q = flow.rates[map.pump_edge];
    assert!(
        flow.rates
            .iter()
            .chain(&flow.pressures)
            .all(|x| x.is_finite())
    );
    let mut csv = String::from(
        "edge,from,to,from_x,from_y,to_x,to_y,diameter,length,signed_unit_rate,rate_over_pump,mean_q_over_d,pressure_from,pressure_to,viscosity,drive\n",
    );
    for (i, e) in map.edges.iter().enumerate() {
        writeln!(
            csv,
            "{i},{},{},{},{},{},{},{},{},{:.9},{:.9},{:.9},{:.9},{:.9},{viscosity},1",
            e.from,
            e.to,
            map.nodes[e.from][0],
            map.nodes[e.from][1],
            map.nodes[e.to][0],
            map.nodes[e.to][1],
            e.half_width * 2.,
            map.edge_length(i),
            flow.rates[i],
            flow.rates[i] / q,
            flow.rates[i] / (e.half_width * 2.),
            flow.pressures[e.from],
            flow.pressures[e.to]
        )
        .unwrap();
    }
    std::fs::write(directory.join("unit-graph.csv"), csv).unwrap();
    let residual = (0..map.nodes.len())
        .map(|n| {
            map.edges
                .iter()
                .zip(&flow.rates)
                .map(|(e, q)| {
                    if e.from == n {
                        *q
                    } else if e.to == n {
                        -*q
                    } else {
                        0.
                    }
                })
                .sum::<f32>()
                .abs()
        })
        .fold(0_f32, f32::max);
    println!(
        "unit_graph nodes={} edges={} pump_edge={} viscosity={viscosity} drive=1 pump_Q={q:.9} max_node_residual={residual:.9}",
        map.nodes.len(),
        map.edges.len(),
        map.pump_edge
    );
    println!(
        "lung_lower_north_Q={:.9} south_Q={:.9} combined_fraction={:.9}",
        flow.rates[29],
        flow.rates[30],
        (flow.rates[29] + flow.rates[30]) / q
    );
}
fn main() {
    let map = MapDefinition::organ_world();
    let cases = cases();
    // Preflight every planned continuous segment before constructing any physics world.
    for case in &cases {
        let mut min = f32::INFINITY;
        for (segment, pair) in case.points.windows(2).enumerate() {
            let distance = (pair[1][0] - pair[0][0]).hypot(pair[1][1] - pair[0][1]);
            let samples = (distance / 0.1).ceil() as usize;
            for i in 0..=samples {
                let t = i as f32 / samples as f32;
                let p = [
                    pair[0][0] * (1. - t) + pair[1][0] * t,
                    pair[0][1] * (1. - t) + pair[1][1] * t,
                ];
                let c = clearance(&map, p);
                assert!(
                    c.is_finite() && c >= 1.,
                    "preflight case={} segment={segment} pose={p:?} clearance={c}",
                    case.name
                );
                min = min.min(c);
            }
        }
        println!(
            "preflight case={} segments={} min_clearance={min:.6} sample_spacing<=0.1",
            case.name,
            case.points.len() - 1
        );
    }
    if let Some(directory) = std::env::args().nth(1) {
        export_graph(&map, Path::new(&directory));
    }
    let mut total = 0;
    for (case_index, case) in cases.iter().enumerate() {
        let mut config = Config {
            robot_speed: 8.,
            pressure: 0.,
            pulse_hz: 0.,
            cell_count: 0,
            ..Default::default()
        };
        config.white_cells.count = 0;
        let mut sim = if case_index == 0 {
            Simulation::new_organ_preview(config)
        } else {
            let mut local = map.clone();
            local.spawn = case.points[0];
            Simulation::new_streamed_world(config, local)
        };
        assert_eq!(sim.snapshot().robot, case.points[0]);
        let mut ticks = 0;
        let mut min = f32::INFINITY;
        let mut peak = 0;
        for (segment, &target) in case.points.iter().enumerate().skip(1) {
            let mut reached = false;
            for _ in 0..2400 {
                let state = sim.snapshot();
                let d = [target[0] - state.robot[0], target[1] - state.robot[1]];
                let length = d[0].hypot(d[1]);
                if length < 0.1 {
                    reached = true;
                    break;
                }
                sim.tick(MoveInput {
                    x: d[0] / length * (length * 2.).min(1.),
                    y: d[1] / length * (length * 2.).min(1.),
                });
                ticks += 1;
                total += 1;
                let s = sim.snapshot();
                let c = clearance(&map, s.robot);
                assert!(
                    total <= 25000
                        && s.robot.iter().all(|v| v.is_finite())
                        && c.is_finite()
                        && c >= 1.,
                    "motion case={} segment={segment} ticks={ticks} pose={:?} clearance={c}",
                    case.name,
                    s.robot
                );
                assert!(
                    s.cells.is_empty()
                        && s.immunity.immune_bodies == 0
                        && s.branch_flows.iter().all(|q| *q == 0.)
                        && s.flow.rate == 0.
                        && s.flow.pressure_drop == 0.
                        && s.flow.mean_speed == 0.
                        && sim.config.pressure == 0.
                        && sim.config.pulse_hz == 0.,
                    "zero-state case={} segment={segment} pose={:?} clearance={c}",
                    case.name,
                    s.robot
                );
                let chunks = s.streaming.unwrap().loaded_chunks;
                assert_eq!(
                    chunks, 25,
                    "case={} segment={segment} pose={:?}",
                    case.name, s.robot
                );
                peak = peak.max(chunks);
                min = min.min(c);
            }
            assert!(
                reached,
                "timeout case={} segment={segment} pose={:?} target={target:?} clearance={}",
                case.name,
                sim.snapshot().robot,
                clearance(&map, sim.snapshot().robot)
            );
            println!(
                "reached case={} segment={segment} pose={:?}",
                case.name,
                sim.snapshot().robot
            );
        }
        println!(
            "PASS case={} ticks={ticks} simulation_seconds={:.3} min_actual_clearance={min:.6} peak_chunks={peak} zero_state=true",
            case.name,
            ticks as f32 * sim.config.dt
        );
    }
    println!(
        "PASS total_ticks={total} total_bound=25000 robot_speed=8 dt=1/60 production_robot_radius=0.85"
    );
}
