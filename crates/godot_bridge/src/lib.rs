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
    error: Option<String>,
    validation_initial_centers: Option<Vec<[f32; 2]>>,
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for BloodLab {
    fn init(base: Base<RefCounted>) -> Self {
        Self {
            simulation: Simulation::default(),
            error: None,
            validation_initial_centers: None,
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
        self.validation_initial_centers = None;
    }

    #[func]
    fn use_local_world(&mut self) {
        self.simulation = Simulation::new_local_world(self.simulation.config.clone());
        self.validation_initial_centers = None;
    }

    #[func]
    fn use_lbm_world(&mut self, profile: GString) -> bool {
        let result = self.create_lbm_world(&profile.to_string());
        self.record_result(result)
    }

    #[func]
    fn use_lbm_validation(&mut self, profile: GString) -> bool {
        let result = self.create_lbm_validation(&profile.to_string());
        self.record_result(result)
    }

    #[func]
    fn reset_lbm_profile(&mut self, profile: GString) -> bool {
        let result = self.reset_profile(&profile.to_string());
        self.record_result(result)
    }

    #[func]
    fn tick(&mut self, movement: Vector2) {
        let result = self.simulation.try_tick(MoveInput {
            x: movement.x,
            y: movement.y,
        });
        if let Err(error) = result {
            self.fail(error);
        }
    }

    #[func]
    fn set_paused(&mut self, paused: bool) {
        if paused || self.error.is_none() {
            self.simulation.set_paused(paused);
        }
    }

    #[func]
    fn single_step(&mut self, movement: Vector2) {
        if self.error.is_none() {
            let result = self.simulation.try_single_step(MoveInput {
                x: movement.x,
                y: movement.y,
            });
            if let Err(error) = result {
                self.fail(error);
            }
        }
    }

    #[func]
    fn reset(&mut self) {
        let result = self.simulation.try_reset();
        self.record_result(result);
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
        let validation = self.validation_snapshot(&state);
        let cells: Array<PackedVector2Array> = state.cells.iter().map(|p| points(p)).collect();
        let arrows: Array<PackedVector2Array> = state
            .arrows
            .iter()
            .map(|arrow| points(&[arrow.position, arrow.velocity]))
            .collect();
        let branch_flows: Array<f32> = state.branch_flows.iter().copied().collect();
        let lbm = state.lbm.as_ref().map(|d| {
            let targets: Array<f32> = d.opening_targets.iter().copied().collect();
            let fluxes: Array<f32> = d.opening_fluxes.iter().copied().collect();
            let residuals: Array<f32> = d.opening_residuals.iter().copied().collect();
            let warmup: Array<Vector3> = d.warmup.iter().map(|v| Vector3::new(v[0], v[1], v[2])).collect();
            vdict! {
                "profile" => d.profile.as_str(),
                "origin_blocks" => Vector2i::new(d.origin_blocks[0], d.origin_blocks[1]),
                "bounds" => &points(&d.bounds),
                "opening_targets" => &targets, "opening_fluxes" => &fluxes,
                "opening_residuals" => &residuals,
                "density_min" => d.density_min, "density_max" => d.density_max, "mass" => d.mass,
                "region_switches" => d.region_switches as i64, "overlap_error" => d.overlap_error,
                "center" => Vector3::new(d.center[0], d.center[1], d.center[2]), "warmup" => &warmup,
                "lattice_steps_per_substep" => d.lattice_steps_per_substep as i64,
                "lattice_dt" => d.lattice_dt, "elapsed_fluid_time" => d.elapsed_fluid_time,
                "coupling_ms" => d.coupling_ms, "lbm_ms" => d.lbm_ms, "rapier_ms" => d.rapier_ms,
                "adapter_name" => d.adapter_name.as_str(), "software_adapter" => d.software_adapter,
            }
        }).unwrap_or_default();
        vdict! {
            "lbm" => &lbm,
            "validation" => &validation,
            "lbm_status" => state.lbm_status.as_deref().unwrap_or(""),
            "error" => self.error.as_deref().unwrap_or(""),
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

impl BloodLab {
    fn fail(&mut self, error: String) {
        godot_error!("Simulation error: {}", error);
        self.simulation.set_paused(true);
        self.error = Some(error);
    }

    fn record_result(&mut self, result: Result<(), String>) -> bool {
        match result {
            Ok(()) => {
                self.error = None;
                true
            }
            Err(error) => {
                self.fail(error);
                false
            }
        }
    }

    #[cfg(feature = "lbm-gpu")]
    fn profile(profile: &str) -> Result<simulation::GridProfile, String> {
        match profile {
            "fine" => Ok(simulation::GridProfile::Fine),
            "coarse" => Ok(simulation::GridProfile::Coarse),
            _ => Err(format!(
                "Unknown LBM profile: {profile}; expected fine or coarse"
            )),
        }
    }

    #[cfg(feature = "lbm-gpu")]
    fn create_lbm_world(&mut self, profile: &str) -> Result<(), String> {
        let options = simulation::LbmOptions {
            profile: Self::profile(profile)?,
            ..Default::default()
        };
        self.simulation = Simulation::new_lbm_world(self.simulation.config.clone(), options)?;
        self.validation_initial_centers = None;
        Ok(())
    }
    #[cfg(not(feature = "lbm-gpu"))]
    fn create_lbm_world(&mut self, _profile: &str) -> Result<(), String> {
        Err("LBM unavailable: build godot_bridge with the lbm-gpu feature".into())
    }
    #[cfg(feature = "lbm-gpu")]
    fn create_lbm_validation(&mut self, profile: &str) -> Result<(), String> {
        let options = simulation::LbmOptions {
            profile: Self::profile(profile)?,
            ..Default::default()
        };
        self.simulation =
            Simulation::new_lbm_validation(simulation::lbm_validation::config(), options)?;
        self.validation_initial_centers = Some(simulation::lbm_validation::cell_centers(
            &self.simulation.snapshot(),
        ));
        Ok(())
    }
    #[cfg(not(feature = "lbm-gpu"))]
    fn create_lbm_validation(&mut self, _profile: &str) -> Result<(), String> {
        Err("LBM unavailable: build godot_bridge with the lbm-gpu feature".into())
    }
    #[cfg(feature = "lbm-gpu")]
    fn validation_snapshot(&self, state: &simulation::Snapshot) -> VarDictionary {
        let Some(initial) = &self.validation_initial_centers else {
            return VarDictionary::new();
        };
        let q = simulation::lbm_validation::quality(
            state,
            self.simulation.world_map().unwrap(),
            &self.simulation.config,
            initial,
        );
        let s = self.simulation.lbm_stats().unwrap();
        let w = self.simulation.lbm_work_stats().unwrap();
        vdict! {"finite"=>q.finite,"area_ratio_min"=>q.area_ratio_min,"area_ratio_max"=>q.area_ratio_max,
        "node_wall_max"=>q.node_wall_max,"robot_wall"=>q.robot_wall,"nearby_cells8"=>q.nearby_cells as i64,
        "cell_displacement_mean"=>q.cell_displacement_mean,"cell_displacement_max"=>q.cell_displacement_max,
        "guard_error"=>simulation::lbm_validation::guard(q).err().unwrap_or_default(),
        "active_cells"=>s.active_cell_count as i64,"marker_force"=>s.max_cell_marker_force,"prepare_ms"=>s.prepare_ms,
        "lattice_steps"=>w.lattice_steps as i64,"dispatches"=>w.dispatches as i64,"submissions"=>w.submissions as i64,
        "readback_calls"=>w.readback_calls as i64,"readback_bytes"=>w.readback_bytes as i64,"readback_wait_ms"=>w.readback_wait_ms}
    }
    #[cfg(not(feature = "lbm-gpu"))]
    fn validation_snapshot(&self, _state: &simulation::Snapshot) -> VarDictionary {
        VarDictionary::new()
    }
    #[cfg(feature = "lbm-gpu")]
    fn reset_profile(&mut self, profile: &str) -> Result<(), String> {
        self.simulation
            .try_reset_lbm_profile(Self::profile(profile)?)
    }
    #[cfg(not(feature = "lbm-gpu"))]
    fn reset_profile(&mut self, _profile: &str) -> Result<(), String> {
        Err("LBM unavailable: build godot_bridge with the lbm-gpu feature".into())
    }
}
