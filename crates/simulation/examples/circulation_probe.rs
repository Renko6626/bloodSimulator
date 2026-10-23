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
    let config = Config {
        cell_count: count,
        ..Config::default()
    };
    let steps = (seconds / config.dt).ceil() as usize;
    let world = args.get(3).is_some_and(|mode| mode == "world");
    let init_started = Instant::now();
    let mut sim = if world {
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
}
