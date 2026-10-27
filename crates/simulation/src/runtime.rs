use crate::{
    Config, FlowArrow, Snapshot, cell,
    circulation::{
        self, Flow, Network,
        graph::{self, GraphFlow, GraphNetwork},
    },
    fluid::{self, LocalRegions},
    map::{MapDefinition, MapGeometry},
    robot,
    vessel::Vessel,
};
use rapier2d::prelude::*;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Default)]
pub struct MoveInput {
    pub x: f32,
    pub y: f32,
}

/// Each environment owns its map and circulation. Physics only samples velocity;
/// a future local fluid solver can evolve here without duplicating body stepping.
enum Environment {
    Loop {
        vessel: Vessel,
        network: Network,
        flow: Flow,
    },
    World {
        map: MapDefinition,
        geometry: MapGeometry,
        network: GraphNetwork,
        flow: GraphFlow,
    },
}

impl Environment {
    fn velocity_at(&self, position: Vector) -> Vector {
        match self {
            Self::Loop { vessel, flow, .. } => {
                circulation::sample_velocity(position, vessel, *flow)
            }
            Self::World { map, flow, .. } => graph::sample_velocity(position, map, flow),
        }
    }

    fn update(&mut self, pressure: f32) {
        match self {
            Self::Loop { network, flow, .. } => *flow = network.solve(pressure),
            Self::World { network, flow, .. } => *flow = network.solve(pressure),
        }
    }

    fn summary(&self) -> Flow {
        match self {
            Self::Loop { flow, .. } => *flow,
            Self::World { flow, .. } => flow.summary,
        }
    }

    fn arrows(&self) -> Vec<FlowArrow> {
        let positions: Vec<Vector> = match self {
            Self::Loop { vessel, .. } => (0..28)
                .map(|i| vessel.at(vessel.length() * i as f32 / 28.0, 0.0))
                .collect(),
            Self::World { map, .. } => (0..map.edges.len())
                .flat_map(|edge| {
                    let length = map.edge_length(edge);
                    let count = (length / 3.0).ceil().max(1.0) as usize;
                    (0..count).map(move |i| {
                        map.edge_point(edge, length * (i as f32 + 0.5) / count as f32, 0.0)
                    })
                })
                .collect(),
        };
        positions
            .into_iter()
            .map(|position| FlowArrow {
                position: position.to_array(),
                velocity: self.velocity_at(position).to_array(),
            })
            .collect()
    }
}

pub struct Simulation {
    pub config: Config,
    physics: PhysicsWorld,
    environment: Environment,
    robot: RigidBodyHandle,
    cells: Vec<SoftBodyHandle>,
    walls: Vec<Vec<[f32; 2]>>,
    steps: u64,
    paused: bool,
    solve_ms: f64,
    local_fluid: Option<LocalRegions>,
    #[cfg(feature = "lbm-gpu")]
    lbm: Option<crate::fluid::lbm_coupling::LbmCoupling>,
}

impl Default for Simulation {
    fn default() -> Self {
        Self::new(Config::default())
    }
}

impl Simulation {
    /// Original closed-loop physics experiment.
    pub fn new(config: Config) -> Self {
        let vessel = Vessel::default();
        let network = Network::new(&vessel, config.viscosity);
        let flow = network.solve(config.pressure);
        Self::build(
            config,
            Environment::Loop {
                vessel,
                network,
                flow,
            },
        )
    }

    pub fn new_world(config: Config) -> Self {
        let map = MapDefinition::small_world();
        let geometry = map.build_geometry(0.2);
        let network = GraphNetwork::new(&map, config.viscosity);
        let flow = network.solve(config.pressure);
        Self::build(
            config,
            Environment::World {
                map,
                geometry,
                network,
                flow,
            },
        )
    }

    #[cfg(feature = "lbm-gpu")]
    pub fn new_lbm_world(config: Config, options: crate::LbmOptions) -> Result<Self, String> {
        Self::new_lbm_world_with_map(config, options, MapDefinition::small_world())
    }

    #[cfg(feature = "lbm-gpu")]
    pub fn new_lbm_world_with_map(
        config: Config,
        options: crate::LbmOptions,
        map: MapDefinition,
    ) -> Result<Self, String> {
        if !config.dt.is_finite()
            || config.dt <= 0.
            || config.substeps == 0
            || !config.viscosity.is_finite()
            || config.viscosity <= 0.
        {
            return Err("invalid mechanical time step".into());
        }
        let geometry = map.build_geometry(0.2);
        let network = GraphNetwork::new(&map, config.viscosity);
        let flow = network.solve(config.pressure);
        let lbm = crate::fluid::lbm_coupling::LbmCoupling::new(&map, &flow, &config, options)?;
        let mut sim = Self::build(
            config,
            Environment::World {
                map,
                geometry,
                network,
                flow,
            },
        );
        sim.lbm = Some(lbm);
        Ok(sim)
    }

    #[cfg(feature = "lbm-gpu")]
    pub fn lbm_stats(&self) -> Option<&crate::LbmStats> {
        self.lbm.as_ref().map(|f| &f.stats)
    }

    /// Two precomputed 64² Stokes regions covering the first fork and upper arm.
    pub fn new_local_world(config: Config) -> Self {
        Self::new_local_world_with_options(config, crate::LocalFluidOptions::default())
    }

    pub fn new_local_world_with_options(config: Config, options: crate::LocalFluidOptions) -> Self {
        let mut sim = Self::new_world(config);
        let Environment::World { map, network, .. } = &sim.environment else {
            unreachable!()
        };
        let mut fluid = LocalRegions::new(
            map,
            &network.solve(1.0),
            sim.config.viscosity,
            sim.cells.len(),
            options,
        )
        .expect("construct local Stokes regions");
        fluid.prepare_pressure(sim.config.pressure);
        sim.physics
            .integration_parameters
            .normalized_prediction_distance = 0.0311;
        sim.local_fluid = Some(fluid);
        sim
    }

    fn build(config: Config, environment: Environment) -> Self {
        assert!(config.dt.is_finite() && config.dt > 0.0 && config.substeps > 0);
        let mut physics = PhysicsWorld::new();
        physics.gravity = Vector::ZERO;
        physics.integration_parameters.dt = config.dt / config.substeps as f32;
        let (walls, spawn, centers) = match &environment {
            Environment::Loop { vessel, .. } => {
                let rows = config.cell_count.div_ceil(3).max(1);
                let centers = (0..config.cell_count)
                    .map(|i| {
                        let lane = i % 3;
                        let s = 3.0
                            + (i / 3) as f32 * (vessel.length() - 6.0) / rows as f32
                            + lane as f32 * (vessel.length() - 6.0) / (3 * rows) as f32;
                        vessel.at(s, (lane as f32 - 1.0) * 1.2)
                    })
                    .collect();
                (vessel.wall_contours(), vessel.at(0.0, 0.0), centers)
            }
            Environment::World { map, geometry, .. } => (
                geometry.walls.clone(),
                Vector::from_array(map.spawn),
                world_cell_centers(map, &config),
            ),
        };
        for contour in &walls {
            for edge in contour.windows(2) {
                if edge[0] == edge[1] {
                    continue;
                }
                physics.insert_collider(
                    ColliderBuilder::capsule_from_endpoints(
                        Vector::from_array(edge[0]),
                        Vector::from_array(edge[1]),
                        0.08,
                    )
                    .friction(0.05),
                    None,
                );
            }
        }
        let robot = robot::insert(&mut physics, spawn);
        let cells = centers
            .into_iter()
            .map(|center| cell::insert(&mut physics, center, &config))
            .collect();
        Self {
            config,
            physics,
            environment,
            robot,
            cells,
            walls,
            steps: 0,
            paused: false,
            solve_ms: 0.0,
            local_fluid: None,
            #[cfg(feature = "lbm-gpu")]
            lbm: None,
        }
    }

    /// Loop-specific diagnostic geometry. World callers use `world_map`.
    pub fn vessel(&self) -> &Vessel {
        match &self.environment {
            Environment::Loop { vessel, .. } => vessel,
            Environment::World { .. } => {
                panic!("vessel() is only available in the loop experiment; use world_map()")
            }
        }
    }

    pub fn world_map(&self) -> Option<&MapDefinition> {
        match &self.environment {
            Environment::World { map, .. } => Some(map),
            Environment::Loop { .. } => None,
        }
    }

    pub fn world_geometry(&self) -> Option<&MapGeometry> {
        match &self.environment {
            Environment::World { geometry, .. } => Some(geometry),
            Environment::Loop { .. } => None,
        }
    }

    pub fn geometry(&self) -> &[Vec<[f32; 2]>] {
        &self.walls
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub fn tick(&mut self, input: MoveInput) {
        if !self.paused {
            self.step(input);
        }
    }

    pub fn single_step(&mut self, input: MoveInput) {
        if self.paused {
            self.step(input);
        }
    }

    pub fn reset(&mut self) {
        #[cfg(feature = "lbm-gpu")]
        if let Some(lbm) = &self.lbm {
            let options = lbm.options.clone();
            let map = self.world_map().unwrap().clone();
            let paused = self.paused;
            *self = Self::new_lbm_world_with_map(self.config.clone(), options, map)
                .expect("reset LBM world");
            self.paused = paused;
            return;
        }
        let paused = self.paused;
        let local = self.local_fluid.as_ref().map(|f| f.options.clone());
        *self = match self.environment {
            Environment::Loop { .. } => Self::new(self.config.clone()),
            Environment::World { .. } if local.is_some() => {
                Self::new_local_world_with_options(self.config.clone(), local.unwrap())
            }
            Environment::World { .. } => Self::new_world(self.config.clone()),
        };
        self.paused = paused;
    }

    fn step(&mut self, input: MoveInput) {
        #[cfg(feature = "lbm-gpu")]
        if let Some(lbm) = &self.lbm {
            lbm.validate_mechanical_time(&self.config)
                .expect("invalid LBM timing");
        }
        let started = Instant::now();
        crate::threading::install(|| self.step_inner(input));
        self.solve_ms = started.elapsed().as_secs_f64() * 1000.0;
    }

    fn step_inner(&mut self, input: MoveInput) {
        let time = self.steps as f32 * self.config.dt;
        let pressure = self.config.pressure
            * (1.0 + 0.15 * (std::f32::consts::TAU * self.config.pulse_hz * time).sin());
        self.environment.update(pressure);
        if let Some(fluid) = &mut self.local_fluid {
            fluid.begin_frame();
            fluid.prepare_pressure(pressure);
        }
        #[cfg(feature = "lbm-gpu")]
        if let Some(lbm) = &mut self.lbm {
            lbm.window.begin_frame();
            lbm.stats.coupling_ms = 0.;
            lbm.stats.lbm_ms = 0.;
            lbm.stats.rapier_ms = 0.;
        }
        let dt = self.config.dt / self.config.substeps as f32;
        self.physics.integration_parameters.dt = dt;
        for _ in 0..self.config.substeps {
            #[cfg(feature = "lbm-gpu")]
            if let (Some(lbm), Environment::World { map, flow, .. }) =
                (&mut self.lbm, &self.environment)
            {
                lbm.prepare(map, flow, self.physics.bodies[self.robot].translation())
                    .expect("prepare LBM window");
            }
            if let Some(fluid) = &mut self.local_fluid {
                fluid
                    .prepare_step(
                        &self.physics,
                        &self.cells,
                        self.robot,
                        self.config.cell_drag,
                        self.config.cell_radius,
                    )
                    .expect("prepare local Stokes region");
            }
            let mut flow_velocity = self
                .environment
                .velocity_at(self.physics.bodies[self.robot].translation());
            if let Some(fluid) = &self.local_fluid {
                let position = self.physics.bodies[self.robot].translation();
                let weight = fluid.weight(position, 0.85);
                flow_velocity = flow_velocity * (1.0 - weight) + fluid.sample(position) * weight;
            }
            #[cfg(feature = "lbm-gpu")]
            if let (Some(lbm), Environment::World { map, .. }) = (&self.lbm, &self.environment) {
                let (w, local, valid) =
                    lbm.local(map, self.physics.bodies[self.robot].translation());
                flow_velocity = if valid {
                    flow_velocity * (1. - w) + local * w
                } else {
                    Vector::ZERO
                };
            }
            robot::drive(
                &mut self.physics.bodies[self.robot],
                input,
                flow_velocity,
                &self.config,
            );
            for &handle in &self.cells {
                #[cfg(feature = "lbm-gpu")]
                if let (Some(lbm), Environment::World { map, .. }) = (&self.lbm, &self.environment)
                {
                    let body = &mut self.physics.soft_bodies[handle];
                    let response = 1. - (-self.config.cell_drag * dt).exp();
                    for i in 0..body.num_particles() {
                        let p = body.particle_position(i);
                        let (w, _, valid) = lbm.local(map, p);
                        let v = body.particle_velocity(i);
                        let target = self.environment.velocity_at(p);
                        if valid {
                            body.set_particle_velocity(i, v + (target - v) * response * (1. - w));
                        }
                    }
                    continue;
                }
                let local_weight = self.local_fluid.as_ref().map_or(0.0, |local| {
                    local.weight(
                        fluid::center(&self.physics.soft_bodies[handle]),
                        self.config.cell_radius,
                    )
                });
                cell::advect(
                    &mut self.physics.soft_bodies[handle],
                    self.config.cell_drag * (1.0 - local_weight),
                    dt,
                    |position| self.environment.velocity_at(position),
                );
            }
            if let Some(fluid) = &mut self.local_fluid {
                fluid
                    .advance(
                        &mut self.physics,
                        &self.cells,
                        self.robot,
                        dt,
                        self.config.cell_drag,
                        self.config.cell_radius,
                    )
                    .expect("local Stokes step");
            }
            #[cfg(feature = "lbm-gpu")]
            if let (Some(lbm), Environment::World { map, .. }) = (&mut self.lbm, &self.environment)
            {
                lbm.advance(
                    map,
                    &mut self.physics,
                    &self.cells,
                    self.robot,
                    dt,
                    self.config.cell_drag,
                )
                .expect("advance LBM window");
            }
            #[cfg(feature = "lbm-gpu")]
            let rapier_started = Instant::now();
            self.physics.step();
            #[cfg(feature = "lbm-gpu")]
            if let Some(lbm) = &mut self.lbm {
                lbm.stats.rapier_ms += rapier_started.elapsed().as_secs_f64() * 1000.;
            }
        }
        self.steps += 1;
    }

    pub fn snapshot(&self) -> Snapshot {
        let soft_contacts = self
            .cells
            .iter()
            .map(|&handle| {
                let body = &self.physics.soft_bodies[handle];
                body.edge_contact_segments(&self.physics.soft_bodies)
                    .count()
                    + body
                        .vertex_contact_segments(&self.physics.soft_bodies)
                        .count()
            })
            .sum();
        let mut arrows = self.environment.arrows();
        if let Some(fluid) = &self.local_fluid {
            arrows.extend(fluid.positions().map(|position| FlowArrow {
                position: position.to_array(),
                velocity: [0.0; 2],
            }));
            for arrow in &mut arrows {
                let position = Vector::from_array(arrow.position);
                let weight = fluid.weight(position, 0.7);
                let velocity = self.environment.velocity_at(position) * (1.0 - weight)
                    + fluid.sample(position) * weight;
                arrow.velocity = velocity.to_array();
            }
        }
        #[cfg(feature = "lbm-gpu")]
        if let (Some(lbm), Environment::World { map, .. }) = (&self.lbm, &self.environment) {
            for arrow in &mut arrows {
                let p = Vector::from_array(arrow.position);
                let (w, u, valid) = lbm.local(map, p);
                arrow.velocity = if valid {
                    (self.environment.velocity_at(p) * (1. - w) + u * w).to_array()
                } else {
                    [0.; 2]
                };
            }
        }
        Snapshot {
            robot: self.physics.bodies[self.robot].translation().to_array(),
            cells: self
                .cells
                .iter()
                .map(|&handle| {
                    self.physics.soft_bodies[handle]
                        .particle_positions()
                        .map(|p| p.to_array())
                        .collect()
                })
                .collect(),
            arrows,
            steps: self.steps,
            time: self.steps as f32 * self.config.dt,
            paused: self.paused,
            solve_ms: self.solve_ms,
            soft_contacts,
            flow: self.environment.summary(),
            branch_flows: match &self.environment {
                Environment::World { flow, .. } => flow.rates.clone(),
                Environment::Loop { .. } => Vec::new(),
            },
            fluid: self.local_fluid.as_ref().map(|f| f.stats().clone()),
        }
    }
}

/// Spread the initial population over all routes, leaving junctions and the
/// robot clear. This placement is only used when constructing/resetting a scene.
fn world_cell_centers(map: &MapDefinition, config: &Config) -> Vec<Vector> {
    if config.cell_count == 0 {
        return Vec::new();
    }
    let spawn = Vector::from_array(map.spawn);
    let spacing = config.cell_radius * 2.0 + 0.3;
    let mut candidates: Vec<Vector> = Vec::new();
    for edge in 0..map.edges.len() {
        let length = map.edge_length(edge);
        let count = (length / spacing).floor().max(1.0) as usize;
        for i in 0..count {
            let along = length * (i as f32 + 0.5) / count as f32;
            let position = map.edge_point(edge, along, 0.0);
            if map.signed_distance(position) > -config.cell_radius - 0.16
                || (position - spawn).length() < config.cell_radius + 0.85 + 0.5
                || candidates
                    .iter()
                    .any(|p| (*p - position).length() < spacing)
            {
                continue;
            }
            candidates.push(position);
        }
    }
    assert!(
        candidates.len() >= config.cell_count,
        "world sample has room for {} cells at this radius; requested {}",
        candidates.len(),
        config.cell_count
    );
    (0..config.cell_count)
        .map(|i| candidates[i * candidates.len() / config.cell_count])
        .collect()
}

#[cfg(test)]
mod region_tests {
    use super::*;

    #[test]
    fn reset_preserves_explicit_solver_and_grid_options() {
        let options = crate::LocalFluidOptions {
            n: 32,
            h: 0.5,
            solver: std::sync::Arc::new(blood_fluid::cpu::MultigridFactory::default()),
            ..Default::default()
        };
        let mut sim = Simulation::new_local_world_with_options(Config::default(), options);
        let initial = sim.snapshot();
        sim.tick(MoveInput { x: 1., y: 0. });
        sim.reset();
        let restored = sim.snapshot();
        let samples = |s: crate::Snapshot| {
            s.arrows
                .into_iter()
                .map(|a| (a.position, a.velocity))
                .collect::<Vec<_>>()
        };
        assert_eq!(samples(initial), samples(restored));
    }

    #[test]
    fn region_handoff_reuses_bodies_and_preparation_applies_no_impulse() {
        let mut sim = Simulation::new_local_world(Config::default());
        for _ in 0..8 {
            sim.tick(MoveInput::default());
        }
        let handles = sim.cells.clone();
        let map = sim.world_map().unwrap().clone();
        let initial_bounds = sim.snapshot().fluid.unwrap().bounds;
        let upper = map.edge_point(1, map.edge_length(1) * 0.5, 0.0);
        for position in [upper, Vector::new(-16.0, 0.0)] {
            sim.physics.bodies[sim.robot].set_translation(position, true);
            let before: Vec<_> = handles
                .iter()
                .flat_map(|&h| {
                    sim.physics.soft_bodies[h]
                        .particle_positions()
                        .zip(sim.physics.soft_bodies[h].particle_velocities())
                })
                .collect();
            let robot_velocity = sim.physics.bodies[sim.robot].linvel();
            sim.local_fluid
                .as_mut()
                .unwrap()
                .prepare_step(
                    &sim.physics,
                    &sim.cells,
                    sim.robot,
                    sim.config.cell_drag,
                    sim.config.cell_radius,
                )
                .unwrap();
            let after: Vec<_> = handles
                .iter()
                .flat_map(|&h| {
                    sim.physics.soft_bodies[h]
                        .particle_positions()
                        .zip(sim.physics.soft_bodies[h].particle_velocities())
                })
                .collect();
            assert_eq!(
                before, after,
                "field preparation must not move or impulse cells"
            );
            assert_eq!(robot_velocity, sim.physics.bodies[sim.robot].linvel());
            assert_eq!(handles, sim.cells);
            let fluid = sim.local_fluid.as_ref().unwrap();
            assert!(fluid.weight(position, 0.85) > 0.99);
            if position == upper {
                assert_ne!(fluid.stats().bounds, initial_bounds);
            } else {
                assert_eq!(fluid.stats().bounds, initial_bounds);
            }
            let selected = fluid.stats().bounds;
            sim.tick(MoveInput::default());
            let stats = sim.snapshot().fluid.unwrap();
            assert_eq!(stats.bounds, selected);
            assert!(stats.divergence_rms < 1e-8);
            assert!(stats.momentum_rms < 1e-8);
            assert!(stats.exchange_error < 1e-7);
        }
    }
}

#[cfg(all(test, feature = "lbm-gpu"))]
mod lbm_runtime_tests {
    use super::*;
    use crate::{GridProfile, LbmOptions, map::VesselEdge};
    fn map() -> MapDefinition {
        let nodes = vec![[-50., 24.], [100., 24.], [100., 80.], [-50., 80.]];
        let edges = [(0, 1), (1, 2), (2, 3), (3, 0)]
            .into_iter()
            .map(|(from, to)| VesselEdge {
                from,
                to,
                half_width: 3.,
                centerline: vec![nodes[from], nodes[to]],
            })
            .collect();
        MapDefinition {
            nodes,
            edges,
            pump_edge: 0,
            spawn: [24., 24.],
            landmarks: vec![],
        }
    }
    fn options() -> LbmOptions {
        LbmOptions {
            profile: GridProfile::Coarse,
            allow_software: true,
            ..Default::default()
        }
    }
    #[test]
    #[ignore = "explicit software-adapter functional diagnostic"]
    fn lbm_runtime_prepare_pause_and_time() {
        let config = Config {
            cell_count: 1,
            pressure: 0.01,
            ..Default::default()
        };
        let mut sim = Simulation::new_lbm_world_with_map(config, options(), map()).unwrap();
        let handles = sim.cells.clone();
        let robot = sim.robot;
        let before: Vec<_> = sim.physics.soft_bodies[handles[0]]
            .particle_positions()
            .zip(sim.physics.soft_bodies[handles[0]].particle_velocities())
            .collect();
        let robot_before = (
            sim.physics.bodies[robot].translation(),
            sim.physics.bodies[robot].linvel(),
        );
        let Environment::World { map, flow, .. } = &sim.environment else {
            unreachable!()
        };
        let lbm = sim.lbm.as_mut().unwrap();
        lbm.window.begin_frame();
        lbm.prepare(map, flow, Vector::new(40., 24.)).unwrap();
        assert_eq!(
            before,
            sim.physics.soft_bodies[handles[0]]
                .particle_positions()
                .zip(sim.physics.soft_bodies[handles[0]].particle_velocities())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            robot_before,
            (
                sim.physics.bodies[robot].translation(),
                sim.physics.bodies[robot].linvel()
            )
        );
        assert_eq!(handles, sim.cells);
        let (w, _, valid) = lbm.local(map, Vector::new(24., 24.));
        assert!(valid && w > 0.99);
        let (w, _, valid) = lbm.local(map, Vector::new(17., 24.));
        assert!(valid && (w - 0.15625).abs() < 1e-5);
        assert_eq!(lbm.local(map, Vector::new(15., 24.)).0, 0.);
        assert!(!lbm.local(map, Vector::new(24., 30.)).2);
        sim.set_paused(true);
        sim.tick(MoveInput::default());
        assert_eq!(sim.snapshot().steps, 0);
        sim.single_step(MoveInput::default());
        assert_eq!(sim.snapshot().steps, 1);
        assert!((sim.lbm_stats().unwrap().elapsed_fluid_time - sim.config.dt as f64).abs() < 1e-7);
        sim.tick(MoveInput::default());
        assert_eq!(sim.snapshot().steps, 1);

        for change_substeps in [false, true] {
            if change_substeps {
                sim.config.substeps *= 2;
            } else {
                sim.config.dt *= 2.;
            }
            let robot_before = (
                sim.physics.bodies[sim.robot].translation(),
                sim.physics.bodies[sim.robot].linvel(),
            );
            let cell_before: Vec<_> = sim.physics.soft_bodies[sim.cells[0]]
                .particle_positions()
                .zip(sim.physics.soft_bodies[sim.cells[0]].particle_velocities())
                .collect();
            let steps_before = sim.snapshot().steps;
            let time_before = sim.lbm_stats().unwrap().elapsed_fluid_time;
            let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sim.single_step(MoveInput::default())
            }))
            .unwrap_err();
            let message = failure.downcast_ref::<String>().unwrap();
            assert!(message.contains("reset required"));
            assert_eq!(
                robot_before,
                (
                    sim.physics.bodies[sim.robot].translation(),
                    sim.physics.bodies[sim.robot].linvel()
                )
            );
            assert_eq!(
                cell_before,
                sim.physics.soft_bodies[sim.cells[0]]
                    .particle_positions()
                    .zip(sim.physics.soft_bodies[sim.cells[0]].particle_velocities())
                    .collect::<Vec<_>>()
            );
            assert_eq!(steps_before, sim.snapshot().steps);
            assert_eq!(time_before, sim.lbm_stats().unwrap().elapsed_fluid_time);
            sim.reset();
            assert_eq!(sim.world_map().unwrap().spawn, [24., 24.]);
            assert_eq!(
                sim.lbm.as_ref().unwrap().options.profile,
                GridProfile::Coarse
            );
            sim.single_step(MoveInput::default());
            assert_eq!(sim.snapshot().steps, 1);
            assert!(
                (sim.lbm_stats().unwrap().elapsed_fluid_time - sim.config.dt as f64).abs() < 1e-7
            );
        }
    }
    #[test]
    #[ignore = "explicit software-adapter functional diagnostic"]
    fn lbm_exchange_conserves_momentum_and_weights_once() {
        let config = Config {
            cell_count: 1,
            pressure: 0.,
            ..Default::default()
        };
        let mut sim = Simulation::new_lbm_world_with_map(config, options(), map()).unwrap();
        let h = sim.cells[0];
        let body = &mut sim.physics.soft_bodies[h];
        let center = body.particle_positions().sum::<Vector>() / body.num_particles() as f32;
        for i in 0..body.num_particles() {
            body.set_particle_position(
                i,
                body.particle_position(i) + Vector::new(-14., 24.) - center,
            );
            body.set_particle_velocity(i, Vector::X);
        }
        let Environment::World { map, .. } = &sim.environment else {
            unreachable!()
        };
        let lbm = sim.lbm.as_mut().unwrap();
        let fluid_before = Vector::from_array(lbm.window.diagnostic_momentum().unwrap());
        let body = &sim.physics.soft_bodies[h];
        let center = body.particle_positions().sum::<Vector>() / body.num_particles() as f32;
        let expected_weight: f32 = (0..4)
            .map(|k| {
                let p = (body.particle_position(k * body.num_particles() / 4) + center) * 0.5;
                lbm.local(map, p).0
            })
            .sum();
        let dt = sim.config.dt / sim.config.substeps as f32;
        let before = body.particle_velocities().sum::<Vector>() / body.num_particles() as f32;
        let kicks = lbm
            .exchange(
                map,
                &mut sim.physics,
                &sim.cells,
                sim.robot,
                dt,
                sim.config.cell_drag,
            )
            .unwrap();
        let body = &sim.physics.soft_bodies[h];
        let after = body.particle_velocities().sum::<Vector>() / body.num_particles() as f32;
        let impulse = after - before;
        let spacing = lbm.window.profile().cell_spacing();
        let fluid: Vector = kicks.iter().map(|k| Vector::from_array(*k)).sum::<Vector>()
            * (spacing.powi(3) / lbm.window.lattice_dt());
        assert!((impulse.x + 0.25 / 4. * dt * expected_weight).abs() < 1e-6);
        assert!((impulse + fluid).length() < 1e-6);
        lbm.window.apply_kicks(&kicks).unwrap();
        let fluid_after = Vector::from_array(lbm.window.diagnostic_momentum().unwrap());
        assert!((impulse + fluid_after - fluid_before).length() < 1e-5);
    }
    #[test]
    #[ignore = "explicit software-adapter functional diagnostic"]
    fn default_map_lbm_boundary_is_explicit() {
        let config = Config {
            cell_count: 0,
            ..Default::default()
        };
        match Simulation::new_lbm_world(config, options()) {
            Ok(_) => eprintln!("default map LBM window constructed"),
            Err(error) => {
                eprintln!("default map LBM window rejected: {error}");
                assert!(error.contains("opening"), "unexpected failure: {error}");
            }
        }
    }
}
