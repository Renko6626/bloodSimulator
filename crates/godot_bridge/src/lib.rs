use godot::builtin::{PackedVector2Array, VarDictionary, vdict};
use godot::classes::{IRefCounted, RefCounted};
use godot::prelude::*;
use simulation::{MoveInput, Simulation};

struct BloodSimulator;

#[gdextension]
unsafe impl ExtensionLibrary for BloodSimulator {}

/// The only Godot-facing simulation owner. No Godot physics bodies are created.
#[derive(GodotClass)]
#[class(base=RefCounted)]
struct BloodLab {
    simulation: Simulation,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for BloodLab {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            simulation: Simulation::default(),
            base,
        }
    }
}

fn points(source: &[[f32; 2]]) -> PackedVector2Array {
    source.iter().map(|p| Vector2::new(p[0], p[1])).collect()
}

#[godot_api]
impl BloodLab {
    #[func]
    fn use_world(&mut self) {
        self.simulation = Simulation::new_world(self.simulation.config.clone());
    }

    #[func]
    fn tick(&mut self, movement: Vector2) {
        self.simulation.tick(MoveInput {
            x: movement.x,
            y: movement.y,
        });
    }

    #[func]
    fn set_paused(&mut self, paused: bool) {
        self.simulation.set_paused(paused);
    }

    #[func]
    fn single_step(&mut self, movement: Vector2) {
        self.simulation.single_step(MoveInput {
            x: movement.x,
            y: movement.y,
        });
    }

    #[func]
    fn reset(&mut self) {
        self.simulation.reset();
    }

    #[func]
    fn set_parameters(&mut self, pressure: f32, robot_speed: f32, cell_drag: f32) {
        self.simulation.config.pressure = pressure.clamp(0.0, 30.0);
        self.simulation.config.robot_speed = robot_speed.clamp(1.0, 8.0);
        self.simulation.config.cell_drag = cell_drag.clamp(0.0, 40.0);
    }

    #[func]
    fn geometry(&self) -> Array<PackedVector2Array> {
        self.simulation
            .geometry()
            .iter()
            .map(|p| points(p))
            .collect()
    }

    #[func]
    fn world_geometry(&self) -> VarDictionary {
        let Some(geometry) = self.simulation.world_geometry() else {
            return VarDictionary::new();
        };
        let walls: Array<PackedVector2Array> =
            geometry.walls.iter().map(|wall| points(wall)).collect();
        let floor = points(&geometry.floor_vertices);
        let bounds = points(&geometry.bounds);
        let landmarks: Array<VarDictionary> =
            self.simulation
                .world_map()
                .map(|map| {
                    map.landmarks.iter().map(|landmark| vdict! {
                "name" => landmark.name.as_str(),
                "position" => Vector2::new(landmark.position[0], landmark.position[1]),
                "size" => Vector2::new(landmark.size[0], landmark.size[1]),
            }).collect()
                })
                .unwrap_or_default();
        vdict! {
            "walls" => &walls,
            "floor" => &floor,
            "bounds" => &bounds,
            "landmarks" => &landmarks,
        }
    }

    #[func]
    fn snapshot(&self) -> VarDictionary {
        let state = self.simulation.snapshot();
        let cells: Array<PackedVector2Array> = state.cells.iter().map(|p| points(p)).collect();
        let arrows: Array<PackedVector2Array> = state
            .arrows
            .iter()
            .map(|arrow| points(&[arrow.position, arrow.velocity]))
            .collect();
        let branch_flows: Array<f32> = state.branch_flows.iter().copied().collect();
        vdict! {
            "robot" => Vector2::new(state.robot[0], state.robot[1]),
            "cells" => &cells, "arrows" => &arrows,
            "steps" => state.steps as i64, "time" => state.time,
            "paused" => state.paused, "solve_ms" => state.solve_ms,
            "pressure_drop" => state.flow.pressure_drop,
            "flow_rate" => state.flow.rate,
            "mean_speed" => state.flow.mean_speed,
            "resistance" => state.flow.resistance,
            "soft_contacts" => state.soft_contacts as i64,
            "branch_flows" => &branch_flows,
        }
    }
}
