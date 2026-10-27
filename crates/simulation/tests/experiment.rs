use simulation::{MoveInput, Simulation, circulation::Network};

#[test]
fn crowded_cells_advect_without_losing_area_or_crossing_walls() {
    use rapier2d::prelude::Vector;

    let area = |cell: &Vec<[f32; 2]>| {
        (0..cell.len())
            .map(|i| {
                let a = cell[i];
                let b = cell[(i + 1) % cell.len()];
                a[0] * b[1] - a[1] * b[0]
            })
            .sum::<f32>()
            .abs()
            * 0.5
    };
    let mut sim = Simulation::default();
    let initial = sim.snapshot();
    let areas: Vec<_> = initial.cells.iter().map(area).collect();
    let initial_center = initial.cells[0]
        .iter()
        .map(|p| Vector::from_array(*p))
        .sum::<Vector>()
        / initial.cells[0].len() as f32;
    let mut saw_contact = false;
    for _ in 0..600 {
        sim.tick(MoveInput::default());
        let state = sim.snapshot();
        saw_contact |= state.soft_contacts > 0;
        for (i, cell) in state.cells.iter().enumerate() {
            let ratio = area(cell) / areas[i];
            assert!(
                (0.75..=1.25).contains(&ratio),
                "cell {i}, t={}, area ratio={ratio}",
                state.time
            );
            for point in cell {
                let p = Vector::from_array(*point);
                assert!(p.is_finite());
                assert!(sim.vessel().project(p).offset.abs() < sim.vessel().half_width + 0.15);
            }
        }
    }
    let end = sim.snapshot();
    let center = end.cells[0]
        .iter()
        .map(|p| Vector::from_array(*p))
        .sum::<Vector>()
        / end.cells[0].len() as f32;
    assert!((center - initial_center).length() > 5.0);
    assert!(saw_contact);
}

#[test]
fn closed_loop_uses_total_resistance_and_poiseuille_profile() {
    use rapier2d::prelude::Vector;
    use simulation::{circulation::sample_velocity, vessel::Vessel};
    use std::f32::consts::PI;

    // L=4π, r=1, μ=1/8 gives R=4. ΔP=8 gives Q=2.
    let vessel = Vessel {
        straight_length: PI,
        bend_radius: 1.0,
        half_width: 1.0,
    };
    let flow = Network::new(&vessel, 0.125).solve(8.0);
    assert!((flow.resistance - 4.0).abs() < 1e-5);
    assert!((flow.rate - 2.0).abs() < 1e-5);
    for (position, expected) in [
        (Vector::new(1.0, 0.0), Vector::new(4.0 / PI, 0.0)),
        (Vector::new(1.0, 0.5), Vector::new(3.0 / PI, 0.0)),
        (Vector::new(1.0, 1.0), Vector::ZERO),
        (Vector::new(1.0, 2.0), Vector::new(-4.0 / PI, 0.0)),
        (Vector::new(-1.0, 1.0), Vector::new(0.0, -4.0 / PI)),
        (Vector::new(PI + 1.0, 1.0), Vector::new(0.0, 4.0 / PI)),
    ] {
        assert!((sample_velocity(position, &vessel, flow) - expected).length() < 1e-5);
    }
    let stopped = Network::new(&vessel, 0.125).solve(0.0);
    assert_eq!(
        sample_velocity(Vector::new(1.0, 0.0), &vessel, stopped),
        Vector::ZERO
    );
}

#[test]
fn robot_has_no_coasting_and_can_easily_move_against_flow() {
    use simulation::Config;

    let config = Config {
        cell_count: 0,
        pressure: 0.0,
        ..Config::default()
    };
    let step_distance = config.robot_speed * config.dt;
    let mut sim = Simulation::new(config);
    let start = sim.snapshot().robot;
    sim.tick(MoveInput { x: 1.0, y: 0.0 });
    let moved = sim.snapshot().robot;
    assert!((moved[0] - start[0] - step_distance).abs() < 1e-5);
    sim.tick(MoveInput::default());
    assert_eq!(sim.snapshot().robot, moved);

    let mut drifting = Simulation::new(Config {
        cell_count: 0,
        ..Config::default()
    });
    let start = drifting.snapshot().robot[0];
    drifting.tick(MoveInput::default());
    let drift = drifting.snapshot().robot[0] - start;
    assert!(drift > 0.0 && drift < step_distance * 0.2);
    let before_reverse = drifting.snapshot().robot[0];
    drifting.tick(MoveInput { x: -1.0, y: 0.0 });
    assert!(before_reverse - drifting.snapshot().robot[0] > step_distance * 0.8);
}

#[test]
fn pause_single_step_and_reset_use_the_same_clock() {
    for mut sim in [
        Simulation::default(),
        Simulation::new_world(simulation::Config::default()),
        Simulation::new_local_world(simulation::Config::default()),
    ] {
        let initial = sim.snapshot();
        let was_world = sim.world_map().is_some();
        let had_fluid = initial.fluid.is_some();
        sim.set_paused(true);
        sim.tick(MoveInput::default());
        assert_eq!(sim.snapshot().robot, initial.robot);
        assert_eq!(sim.snapshot().steps, 0);
        sim.single_step(MoveInput { x: 1.0, y: 0.0 });
        assert_eq!(sim.snapshot().steps, 1);
        assert!(sim.snapshot().robot[0] > initial.robot[0]);
        if let Some(fluid) = sim.snapshot().fluid {
            assert!(fluid.active_cells > 0);
            assert!(fluid.exchange_error < 1e-7);
            assert!(fluid.divergence_rms < 1e-7);
            assert!(fluid.momentum_rms < 1e-7);
        }
        sim.tick(MoveInput::default());
        assert_eq!(sim.snapshot().steps, 1);
        sim.reset();
        assert_eq!(sim.snapshot().robot, initial.robot);
        assert_eq!(sim.snapshot().cells, initial.cells);
        assert_eq!(sim.snapshot().steps, 0);
        assert!(sim.snapshot().paused);
        assert_eq!(sim.world_map().is_some(), was_world);
        assert_eq!(sim.snapshot().fluid.is_some(), had_fluid);
        assert_eq!(sim.snapshot().branch_flows, initial.branch_flows);
    }
}

#[test]
fn world_cells_pass_both_branches_and_rejoin() {
    use rapier2d::prelude::Vector;

    let mut sim = Simulation::new_world(simulation::Config::default());
    let map = sim.world_map().unwrap().clone();
    let fork = (0..map.nodes.len())
        .find(|&node| map.edges.iter().filter(|edge| edge.from == node).count() == 2)
        .unwrap();
    let join = (0..map.nodes.len())
        .find(|&node| map.edges.iter().filter(|edge| edge.to == node).count() == 2)
        .unwrap();
    let edge_of = |cell: &Vec<[f32; 2]>| {
        let center =
            cell.iter().map(|p| Vector::from_array(*p)).sum::<Vector>() / cell.len() as f32;
        map.nearest_edge(center)
    };
    let mut previous: Vec<_> = sim.snapshot().cells.iter().map(edge_of).collect();
    let mut left_fork = vec![false; map.edges.len()];
    let mut entered_join = vec![false; map.edges.len()];
    for _ in 0..600 {
        sim.tick(MoveInput::default());
        for (i, cell) in sim.snapshot().cells.iter().enumerate() {
            let edge = edge_of(cell);
            if edge != previous[i] && map.edges[previous[i]].to == map.edges[edge].from {
                if map.edges[edge].from == fork {
                    left_fork[edge] = true;
                }
                if map.edges[previous[i]].to == join {
                    entered_join[previous[i]] = true;
                }
            }
            previous[i] = edge;
        }
    }
    for (id, edge) in map.edges.iter().enumerate() {
        if edge.from == fork {
            assert!(left_fork[id], "no cell entered fork branch {id}");
        }
        if edge.to == join {
            assert!(
                entered_join[id],
                "no cell left branch {id} through the join"
            );
        }
    }
}
