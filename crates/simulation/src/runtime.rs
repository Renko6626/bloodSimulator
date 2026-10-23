use crate::{
    Config, FlowArrow, Snapshot, cell,
    circulation::{
        self, Flow, Network,
        graph::{self, GraphFlow, GraphNetwork},
    },
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
        let paused = self.paused;
        *self = match self.environment {
            Environment::Loop { .. } => Self::new(self.config.clone()),
            Environment::World { .. } => Self::new_world(self.config.clone()),
        };
        self.paused = paused;
    }

    fn step(&mut self, input: MoveInput) {
        let started = Instant::now();
        let time = self.steps as f32 * self.config.dt;
        let pressure = self.config.pressure
            * (1.0 + 0.15 * (std::f32::consts::TAU * self.config.pulse_hz * time).sin());
        self.environment.update(pressure);
        let dt = self.config.dt / self.config.substeps as f32;
        self.physics.integration_parameters.dt = dt;
        for _ in 0..self.config.substeps {
            let flow_velocity = self
                .environment
                .velocity_at(self.physics.bodies[self.robot].translation());
            robot::drive(
                &mut self.physics.bodies[self.robot],
                input,
                flow_velocity,
                &self.config,
            );
            for &handle in &self.cells {
                cell::advect(
                    &mut self.physics.soft_bodies[handle],
                    self.config.cell_drag,
                    dt,
                    |position| self.environment.velocity_at(position),
                );
            }
            self.physics.step();
        }
        self.steps += 1;
        self.solve_ms = started.elapsed().as_secs_f64() * 1000.0;
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
            arrows: self.environment.arrows(),
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
