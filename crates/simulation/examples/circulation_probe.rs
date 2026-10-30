//! Real soft-body traffic: cargo run -p simulation --example circulation_probe -- 72 60 [world]
use rapier2d::prelude::Vector;
use simulation::{Config, MoveInput, Simulation};
use std::time::Instant;

fn area(points: &[[f32; 2]]) -> f32 {
    (0..points.len())
        .map(|i| {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            a[0] * b[1] - a[1] * b[0]
        })
        .sum::<f32>()
        .abs()
        * 0.5
}

fn center(points: &[[f32; 2]]) -> Vector {
    points
        .iter()
        .map(|p| Vector::from_array(*p))
        .sum::<Vector>()
        / points.len() as f32
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let count = args
        .get(1)
        .map(|s| s.parse().expect("cell count must be an integer"))
        .unwrap_or(72);
    let seconds: f32 = args
        .get(2)
        .map(|s| s.parse().expect("duration must be a number"))
        .unwrap_or(60.0);
    assert!(count > 0 && seconds.is_finite() && seconds > 0.0);
    if args
        .get(3)
        .is_some_and(|mode| mode == "population" || mode == "population-full")
    {
        population_probe(
            count,
            seconds,
            args[3] == "population",
            args.get(4).map(String::as_str),
        );
        return;
    }
    let config = Config {
        cell_count: count,
        white_cells: simulation::WhiteCellOptions {
            count: 0,
            ..Default::default()
        },
        ..Config::default()
    };
    let steps = (seconds / config.dt).ceil() as usize;
    let local = args.get(3).is_some_and(|mode| mode == "local");
    let world = local || args.get(3).is_some_and(|mode| mode == "world");
    let init_started = Instant::now();
    let mut sim = if local {
        Simulation::new_local_world(config)
    } else if world {
        Simulation::new_world(config)
    } else {
        Simulation::new(config)
    };
    let init_ms = init_started.elapsed().as_secs_f64() * 1000.0;
    if let Some(map) = sim.world_map() {
        let geometry = sim.world_geometry().unwrap();
        println!(
            "world nodes={}, edges={}, wall segments={}, floor triangles={}, init={init_ms:.1}ms",
            map.nodes.len(),
            map.edges.len(),
            geometry.walls.len(),
            geometry.floor_vertices.len() / 3
        );
    }
    let initial = sim.snapshot();
    let initial_areas: Vec<_> = initial.cells.iter().map(|p| area(p)).collect();
    let mut previous_centers: Vec<_> = initial.cells.iter().map(|p| center(p)).collect();
    let mut previous_edges: Vec<_> = sim
        .world_map()
        .map(|map| {
            previous_centers
                .iter()
                .map(|p| map.nearest_edge(*p))
                .collect()
        })
        .unwrap_or_default();
    let mut transitions = sim
        .world_map()
        .map(|map| vec![vec![0usize; map.edges.len()]; map.edges.len()])
        .unwrap_or_default();
    let mut previous: Vec<_> = initial
        .cells
        .iter()
        .map(|p| {
            if world {
                0.0
            } else {
                sim.vessel().project(center(p)).distance
            }
        })
        .collect();
    let mut progress = vec![0.0; count];
    let mut solve_times = Vec::with_capacity(steps);
    let mut fluid_times = Vec::with_capacity(steps);
    let (mut max_divergence, mut max_momentum, mut max_exchange, mut max_net_flux) =
        (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    let (mut min_active, mut max_active) = (usize::MAX, 0);
    let mut fluid_iterations = 0usize;
    let mut contact_steps = 0;
    let mut maximum_area_error = 0.0_f32;
    let mut maximum_wall_overshoot = 0.0_f32;
    let mut maximum_aspect = 1.0_f32;
    let length = if world { 1.0 } else { sim.vessel().length() };
    let started = Instant::now();
    for _ in 0..steps {
        sim.tick(MoveInput::default());
        let state = sim.snapshot();
        solve_times.push(state.solve_ms);
        if let Some(fluid) = &state.fluid {
            fluid_times.push(fluid.solve_ms);
            max_divergence = max_divergence.max(fluid.divergence_rms);
            max_momentum = max_momentum.max(fluid.momentum_rms);
            max_exchange = max_exchange.max(fluid.exchange_error);
            max_net_flux = max_net_flux.max(fluid.net_flux.abs());
            min_active = min_active.min(fluid.active_cells);
            max_active = max_active.max(fluid.active_cells);
            fluid_iterations += fluid.pressure_iterations;
        }
        contact_steps += usize::from(state.soft_contacts > 0);
        for (i, cell) in state.cells.iter().enumerate() {
            assert!(
                cell.iter().all(|p| p.iter().all(|x| x.is_finite())),
                "non-finite cell {i}"
            );
            let current_center = center(cell);
            if let Some(map) = sim.world_map() {
                progress[i] += (current_center - previous_centers[i]).length();
                let edge = map.nearest_edge(current_center);
                if edge != previous_edges[i]
                    && map.edges[previous_edges[i]].to == map.edges[edge].from
                {
                    transitions[previous_edges[i]][edge] += 1;
                }
                previous_edges[i] = edge;
            } else {
                let s = sim.vessel().project(current_center).distance;
                progress[i] += (s - previous[i] + length * 0.5).rem_euclid(length) - length * 0.5;
                previous[i] = s;
            }
            previous_centers[i] = current_center;
            maximum_area_error =
                maximum_area_error.max((area(cell) / initial_areas[i] - 1.0).abs());
            let mut low = Vector::splat(f32::INFINITY);
            let mut high = Vector::splat(f32::NEG_INFINITY);
            for p in cell {
                let p = Vector::from_array(*p);
                low = low.min(p);
                high = high.max(p);
                let overshoot = if let Some(map) = sim.world_map() {
                    map.signed_distance(p)
                } else {
                    sim.vessel().project(p).offset.abs() - sim.vessel().half_width
                };
                maximum_wall_overshoot = maximum_wall_overshoot.max(overshoot);
            }
            let size = high - low;
            maximum_aspect = maximum_aspect.max((size.x / size.y).max(size.y / size.x));
        }
    }
    progress.sort_by(f32::total_cmp);
    solve_times.sort_by(f64::total_cmp);
    let completed = progress.iter().filter(|&&s| s >= length).count();
    println!(
        "cells={count}, simulated={:.1}s, wall_time={:.2}s",
        sim.snapshot().time,
        started.elapsed().as_secs_f64()
    );
    if world {
        println!(
            "distance min/median/max={:.3}/{:.3}/{:.3}",
            progress[0],
            progress[count / 2],
            progress[count - 1]
        );
        for (from, destinations) in transitions.iter().enumerate() {
            let observed: Vec<_> = destinations
                .iter()
                .enumerate()
                .filter(|(_, n)| **n > 0)
                .collect();
            println!("edge {from} downstream crossings: {observed:?}");
        }
    } else {
        println!(
            "laps min/median/max={:.3}/{:.3}/{:.3}, completed={completed}/{count}",
            progress[0] / length,
            progress[count / 2] / length,
            progress[count - 1] / length
        );
    }
    println!(
        "soft-contact steps={contact_steps}/{steps}, max area error={:.2}%, max bbox aspect={maximum_aspect:.3}, max wall overshoot={maximum_wall_overshoot:.4}",
        maximum_area_error * 100.0
    );
    println!(
        "solve mean/p95/max={:.3}/{:.3}/{:.3} ms",
        solve_times.iter().sum::<f64>() / steps as f64,
        solve_times[steps * 95 / 100],
        solve_times[steps - 1]
    );
    if !fluid_times.is_empty() {
        fluid_times.sort_by(f64::total_cmp);
        println!(
            "local fluid mean/p95/max={:.3}/{:.3}/{:.3} ms, active cells={min_active}..{max_active}, pressure iterations/frame={:.2}, div={max_divergence:.3e}, momentum={max_momentum:.3e}, exchange={max_exchange:.3e}, net flux={max_net_flux:.3e}",
            fluid_times.iter().sum::<f64>() / steps as f64,
            fluid_times[(steps * 95).div_ceil(100) - 1],
            fluid_times[steps - 1],
            fluid_iterations as f64 / steps as f64
        );
    }
}

/// Compare real world mechanics using the same ordinary MoveInput route. CSV
/// reports instantaneous local shape/contact quality, never individual paths.
fn population_probe(count: usize, seconds: f32, local: bool, output: Option<&str>) {
    use std::{
        fs::File,
        io::{BufWriter, Write},
    };
    let config = Config {
        cell_count: count,
        white_cells: simulation::WhiteCellOptions {
            count: 0,
            ..Default::default()
        },
        cell_mode: if local {
            simulation::CellSimulationMode::Local(Default::default())
        } else {
            simulation::CellSimulationMode::Full
        },
        ..Default::default()
    };
    let rest_area = 0.5
        * config.cell_vertices as f32
        * config.cell_radius.powi(2)
        * (std::f32::consts::TAU / config.cell_vertices as f32).sin();
    let mut sim = Simulation::new_world(config.clone());
    let steps = (seconds / config.dt).ceil() as usize;
    // Follow centerlines through the upper arm and the return route. This is
    // feedback through robot::drive, with no simulated body teleports.
    let route = [0, 1, 2, 3, 7, 8, 9, 10, 11];
    let map = sim.world_map().unwrap();
    let mut waypoints = vec![Vector::from_array(map.spawn)];
    for edge in route {
        let length = map.edge_length(edge);
        let samples = (length / 1.5).ceil() as usize;
        let start = if edge == 0 {
            map.project_edge(edge, Vector::from_array(map.spawn)).along
        } else {
            0.
        };
        for n in 1..=samples {
            waypoints.push(map.edge_point(
                edge,
                start + (length - start) * n as f32 / samples as f32,
                0.,
            ));
        }
    }
    let mut target = 1;
    let mut csv = output.map(|path| BufWriter::new(File::create(path).expect("create CSV")));
    if let Some(out) = &mut csv {
        writeln!(out,"frame,time,robot_x,robot_y,route_waypoint,total_cells,detailed_cells,abstract_cells,quantity_error,spawned,retired,cell_exchange_ms,solve_ms,snapshot_ms,soft_contacts,area_ratio_min,area_ratio_max,node_wall_max,robot_wall").unwrap();
    }
    let mut state = sim.snapshot();
    let mut solve = Vec::new();
    let mut snapshots = Vec::new();
    let mut exchange = Vec::new();
    let mut handoff_solve = Vec::new();
    let (mut spawned, mut retired, mut contact_frames) = (0, 0, 0);
    let (mut min_detailed, mut max_detailed) = (state.cells.len(), state.cells.len());
    let (mut quantity_error, mut max_area_error, mut max_wall) = (0_f64, 0_f32, f32::NEG_INFINITY);
    for frame in 0..steps {
        let position = Vector::from_array(state.robot);
        if (waypoints[target] - position).length() < 0.7 && target + 1 < waypoints.len() {
            target += 1;
        }
        let movement = ((waypoints[target] - position) * 2.).clamp_length_max(1.);
        sim.tick(MoveInput {
            x: movement.x,
            y: movement.y,
        });
        let start = Instant::now();
        state = sim.snapshot();
        let snapshot_ms = start.elapsed().as_secs_f64() * 1000.;
        assert!(state.robot.iter().all(|v| v.is_finite()));
        let (abstract_count, births, deaths, exchange_ms) = state
            .population
            .as_ref()
            .map(|p| {
                (
                    p.abstract_count,
                    p.spawned_this_frame,
                    p.retired_this_frame,
                    p.exchange_ms,
                )
            })
            .unwrap_or((0., 0, 0, 0.));
        let error = (state.cells.len() as f64 + abstract_count - count as f64).abs();
        assert!(
            error < 1e-6,
            "frame {frame}: cell quantity mismatch {error}"
        );
        quantity_error = quantity_error.max(error);
        min_detailed = min_detailed.min(state.cells.len());
        max_detailed = max_detailed.max(state.cells.len());
        spawned += births;
        retired += deaths;
        contact_frames += usize::from(state.soft_contacts > 0);
        if births + deaths > 0 {
            handoff_solve.push(state.solve_ms);
        }
        solve.push(state.solve_ms);
        snapshots.push(snapshot_ms);
        exchange.push(exchange_ms);
        let (mut area_min, mut area_max, mut wall_max) = (f32::INFINITY, 0_f32, f32::NEG_INFINITY);
        let map = sim.world_map().unwrap();
        for cell in &state.cells {
            let ratio = area(cell) / rest_area;
            area_min = area_min.min(ratio);
            area_max = area_max.max(ratio);
            max_area_error = max_area_error.max((ratio - 1.).abs());
            for p in cell {
                assert!(
                    p.iter().all(|v| v.is_finite()),
                    "frame {frame}: nonfinite cell"
                );
                wall_max = wall_max.max(map.signed_distance(Vector::from_array(*p)));
            }
        }
        max_wall = max_wall.max(wall_max);
        if let Some(out) = &mut csv {
            let measured = |v: f32| {
                if state.cells.is_empty() {
                    String::new()
                } else {
                    v.to_string()
                }
            };
            writeln!(out,"{frame},{},{},{},{target},{count},{},{abstract_count},{error},{births},{deaths},{exchange_ms},{},{snapshot_ms},{},{},{},{},{}",
                state.time,state.robot[0],state.robot[1],state.cells.len(),state.solve_ms,state.soft_contacts,
                measured(area_min),measured(area_max),measured(wall_max),map.signed_distance(Vector::from_array(state.robot))).unwrap();
        }
    }
    if let Some(out) = &mut csv {
        out.flush().unwrap();
    }
    let report = |label: &str, mut values: Vec<f64>| {
        if values.is_empty() {
            println!("{label}: no observed frames");
            return;
        }
        values.sort_by(f64::total_cmp);
        println!(
            "{label} mean/p95/max={:.6}/{:.6}/{:.6}ms n={}",
            values.iter().sum::<f64>() / values.len() as f64,
            values[(values.len() * 95).div_ceil(100) - 1],
            values[values.len() - 1],
            values.len()
        );
    };
    println!(
        "population local={local} total={count} detailed={min_detailed}..{max_detailed} simulated={}s robot={:?} waypoint={target}/{} spawned={spawned} retired={retired} max_quantity_error={quantity_error:.3e} contact_frames={contact_frames}/{steps} max_area_error={max_area_error:.6} max_node_wall={max_wall:.6}",
        state.time,
        state.robot,
        waypoints.len()
    );
    report("solve", solve);
    report("snapshot", snapshots);
    report("cell_exchange", exchange);
    report("handoff_solve", handoff_solve);
}
