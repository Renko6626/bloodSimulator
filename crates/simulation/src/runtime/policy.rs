//! Prepared map policy and the single frame-selected local fluid window.
use super::*;
#[cfg(feature = "lbm-gpu")]
use crate::fluid::{lbm_coupling::LbmCoupling, lbm_policy::PreparedLbmPolicy};
use crate::{
    background::PreparedBackground,
    map::{ChunkId, fluid_policy::BackgroundSource},
};

#[derive(Clone, Debug)]
pub struct PolicyFluidStatus {
    /// Current robot chunk, including movement since the start of this frame.
    pub center: ChunkId,
    pub background: BackgroundSource,
    pub window_active: bool,
    /// Selected at frame start; the active window can lag `center` across an edge.
    /// Use the LBM snapshot bounds to identify that frame's selected window.
    pub activation: f32,
    pub rejection: Option<String>,
}

pub(super) struct PolicyRuntime {
    #[cfg(feature = "lbm-gpu")]
    pub(super) lbm_request: Option<crate::LbmOptions>,
    #[cfg(feature = "lbm-gpu")]
    prepared: Option<PreparedLbmPolicy>,
}

impl PolicyRuntime {
    pub(super) fn select_frame(&self, position: Vector) -> Option<ChunkId> {
        #[cfg(feature = "lbm-gpu")]
        if position.is_finite() {
            let center = ChunkId::at(position.to_array());
            return self
                .prepared
                .as_ref()
                .and_then(|p| p.contains(center).then_some(center));
        }
        #[cfg(not(feature = "lbm-gpu"))]
        let _ = position;
        None
    }
}

#[cfg(feature = "lbm-gpu")]
fn origin(center: ChunkId) -> blood_fluid_gpu::lbm::WindowOrigin {
    // PreparedLbmPolicy has checked the entire neighborhood, including overflow.
    blood_fluid_gpu::lbm::WindowOrigin {
        block_x: center.x - 1,
        block_y: center.y - 1,
    }
}

impl Simulation {
    pub fn try_new_policy_world(config: Config, map: MapDefinition) -> Result<Self, String> {
        Self::new_policy_world(
            config,
            map,
            #[cfg(feature = "lbm-gpu")]
            None,
        )
    }

    #[cfg(feature = "lbm-gpu")]
    pub fn try_new_policy_lbm_world(
        config: Config,
        options: crate::LbmOptions,
        map: MapDefinition,
    ) -> Result<Self, String> {
        Self::new_policy_world(config, map, Some(options))
    }

    fn new_policy_world(
        config: Config,
        map: MapDefinition,
        #[cfg(feature = "lbm-gpu")] options: Option<crate::LbmOptions>,
    ) -> Result<Self, String> {
        let definition = map
            .fluid
            .as_ref()
            .ok_or("policy world requires map.fluid")?;
        if !config.viscosity.is_finite()
            || config.viscosity <= 0.
            || !config.dt.is_finite()
            || config.dt <= 0.
            || config.substeps == 0
            || !config.pressure.is_finite()
            || config.pressure < 0.
            || !definition.max_base_pressure.is_finite()
            || definition.max_base_pressure <= 0.
            || config.pressure > definition.max_base_pressure
        {
            return Err("invalid policy time, viscosity or base pressure range".into());
        }
        let config = streaming::local_config(config);
        let network = GraphNetwork::new(&map, config.viscosity);
        let background = PreparedBackground::new(&map, &config, &network.solve(1.))?;
        #[cfg(feature = "lbm-gpu")]
        let prepared = if let Some(options) = &options {
            LbmCoupling::validate_options(&config, options)?;
            Some(PreparedLbmPolicy::new(
                &map,
                &network.solve(1.15 * definition.max_base_pressure),
                options,
            )?)
        } else {
            None
        };
        let policy = PolicyRuntime {
            #[cfg(feature = "lbm-gpu")]
            lbm_request: options,
            #[cfg(feature = "lbm-gpu")]
            prepared,
        };
        let flow = network.solve(config.pressure);
        // All CPU preparation precedes GPU allocation and body construction.
        #[cfg(feature = "lbm-gpu")]
        let lbm = if let Some(center) = policy.select_frame(Vector::from_array(map.spawn)) {
            let options = policy.lbm_request.as_ref().unwrap();
            let mut lbm =
                LbmCoupling::new_at(&map, &flow, &config, options.clone(), origin(center))?;
            lbm.frame_activation = policy
                .prepared
                .as_ref()
                .unwrap()
                .activation(Vector::from_array(map.spawn), options.outer_band);
            Some(lbm)
        } else {
            None
        };
        let mut sim = Self::build_seeded(
            config,
            Environment::World {
                map,
                geometry: None,
                background: Some(background),
                network,
                flow,
            },
            None,
        );
        sim.policy = Some(policy);
        #[cfg(feature = "lbm-gpu")]
        {
            sim.lbm = lbm;
        }
        sim.enable_streaming();
        sim.seed_white_cells();
        Ok(sim)
    }

    pub fn policy_fluid_status(&self) -> Option<PolicyFluidStatus> {
        let policy = self.policy.as_ref()?;
        let center = ChunkId::at(self.physics.bodies[self.robot].translation().to_array());
        let definition = self.world_map()?.fluid.as_ref()?;
        #[cfg(feature = "lbm-gpu")]
        let (window_active, activation, rejection) = (
            self.lbm.is_some(),
            self.lbm.as_ref().map_or(0., |lbm| lbm.frame_activation),
            policy.prepared.as_ref().and_then(|p| {
                p.rejections()
                    .iter()
                    .find(|r| r.center == center)
                    .map(|r| r.reason.clone())
            }),
        );
        #[cfg(not(feature = "lbm-gpu"))]
        let (window_active, activation, rejection) = {
            let _ = policy.select_frame(self.physics.bodies[self.robot].translation());
            (false, 0., None)
        };
        Some(PolicyFluidStatus {
            center,
            background: definition.policy_at(center).background,
            window_active,
            activation,
            rejection,
        })
    }

    pub(super) fn reset_policy(
        &mut self,
        #[cfg(feature = "lbm-gpu")] profile: Option<crate::GridProfile>,
        #[cfg(not(feature = "lbm-gpu"))] _profile: Option<()>,
    ) -> Result<(), String> {
        let map = self.world_map().unwrap().clone();
        #[cfg(feature = "lbm-gpu")]
        let mut options = self.policy.as_ref().unwrap().lbm_request.clone();
        #[cfg(feature = "lbm-gpu")]
        if let (Some(options), Some(profile)) = (&mut options, profile) {
            options.profile = profile;
        }
        let mut replacement = Self::new_policy_world(
            self.config.clone(),
            map,
            #[cfg(feature = "lbm-gpu")]
            options,
        )?;
        replacement.white_cells_enabled = self.white_cells_enabled;
        replacement.paused = self.paused;
        *self = replacement;
        Ok(())
    }

    #[cfg(feature = "lbm-gpu")]
    pub(super) fn select_policy_frame(&mut self) -> Result<(), String> {
        let Some(policy) = &self.policy else {
            return Ok(());
        };
        let position = self.physics.bodies[self.robot].translation();
        let Some(center) = policy.select_frame(position) else {
            self.lbm = None;
            return Ok(());
        };
        let options = policy.lbm_request.as_ref().unwrap();
        let Environment::World { map, flow, .. } = &self.environment else {
            unreachable!()
        };
        if let Some(lbm) = &mut self.lbm {
            lbm.prepare_at(map, flow, origin(center))?;
        } else {
            let mut lbm =
                LbmCoupling::new_at(map, flow, &self.config, options.clone(), origin(center))?;
            lbm.window.begin_frame();
            self.lbm = Some(lbm);
        }
        self.lbm.as_mut().unwrap().frame_activation = policy
            .prepared
            .as_ref()
            .unwrap()
            .activation(position, options.outer_band);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{
        VesselEdge,
        fluid_policy::{ChunkFluidPolicy, MapFluidDefinition},
    };

    fn fixture() -> (Config, MapDefinition) {
        let nodes = vec![[-50., 24.], [100., 24.], [100., 80.], [-50., 80.]];
        let map = MapDefinition {
            edges: (0..4)
                .map(|from| {
                    let to = (from + 1) % 4;
                    VesselEdge {
                        from,
                        to,
                        half_width: 3.,
                        centerline: vec![nodes[from], nodes[to]],
                    }
                })
                .collect(),
            nodes,
            pump_edge: 2,
            spawn: [24., 24.],
            landmarks: vec![],
            fluid: Some(MapFluidDefinition {
                geometry_revision: 1,
                max_base_pressure: 0.1,
                chunks: ChunkId { x: 1, y: 1 }
                    .neighborhood(1)
                    .into_iter()
                    .map(|chunk| {
                        (
                            chunk,
                            ChunkFluidPolicy {
                                background: BackgroundSource::Analytic,
                                allow_lbm: true,
                            },
                        )
                    })
                    .collect(),
                fields: vec![],
            }),
        };
        let config = Config {
            pressure: 0.01,
            cell_count: 1,
            white_cells: crate::WhiteCellOptions {
                count: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        (config, map)
    }

    #[test]
    fn policy_rejects_drive_before_bodies_time_or_lifetimes_change() {
        let (config, map) = fixture();
        let mut sim = Simulation::try_new_policy_world(config, map).unwrap();
        let cell = sim.insert_white_cell(Vector::new(28., 24.));
        sim.cells[cell].damage(1000.);
        let robot = (
            sim.physics.bodies[sim.robot].translation(),
            sim.physics.bodies[sim.robot].linvel(),
        );
        let body = sim.cells[cell].active_body().unwrap();
        let before: Vec<_> = sim.physics.soft_bodies[body].particle_positions().collect();
        let pressure = sim.environment.summary().pressure_drop;
        for invalid in [0.1001, -0.01, f32::NAN] {
            sim.config.pressure = invalid;
            assert!(sim.try_tick(MoveInput { x: 1., y: 0. }).is_err());
            assert_eq!(
                robot,
                (
                    sim.physics.bodies[sim.robot].translation(),
                    sim.physics.bodies[sim.robot].linvel()
                )
            );
            assert_eq!(
                before,
                sim.physics.soft_bodies[body]
                    .particle_positions()
                    .collect::<Vec<_>>()
            );
            assert_eq!(sim.elapsed_time, 0.);
            assert_eq!(sim.steps, 0);
            assert_eq!(sim.cells[cell].dead_age, Some(0.));
            assert_eq!(sim.environment.summary().pressure_drop, pressure);
        }
    }

    #[test]
    fn policy_valid_drive_advances_and_reset_preserves_map_and_streaming() {
        let (config, map) = fixture();
        let mut sim = Simulation::try_new_policy_world(config, map).unwrap();
        sim.try_tick(MoveInput { x: 1., y: 0. }).unwrap();
        assert_eq!(sim.steps, 1);
        assert!(sim.elapsed_time > 0.);
        assert_ne!(
            sim.physics.bodies[sim.robot].translation(),
            Vector::new(24., 24.)
        );
        sim.set_paused(true);
        sim.try_reset().unwrap();
        assert!(sim.paused);
        assert_eq!(sim.steps, 0);
        assert_eq!(sim.world_map().unwrap().spawn, [24., 24.]);
        assert!(sim.streaming.is_some());
        let status = sim.policy_fluid_status().unwrap();
        assert_eq!(status.center, ChunkId { x: 1, y: 1 });
        assert_eq!(status.background, BackgroundSource::Analytic);
        assert!(!status.window_active);
        sim.config.viscosity *= 2.;
        sim.set_paused(false);
        assert!(sim.try_tick(MoveInput::default()).is_err());
        assert_eq!(sim.steps, 0);
    }

    #[cfg(feature = "lbm-gpu")]
    #[test]
    fn policy_frame_selection_only_uses_prepared_centers_and_retains_inactive_request() {
        let (config, mut map) = fixture();
        // No adapter is requested at this forbidden spawn, despite permission elsewhere.
        map.spawn = [-40., 24.];
        let mut sim = Simulation::try_new_policy_lbm_world(
            config,
            crate::LbmOptions {
                profile: crate::GridProfile::Coarse,
                ..Default::default()
            },
            map,
        )
        .unwrap();
        assert!(sim.lbm.is_none());
        let policy = sim.policy.as_ref().unwrap();
        for p in [Vector::new(24., 24.), Vector::new(31.9, 24.)] {
            assert_eq!(policy.select_frame(p), Some(ChunkId { x: 1, y: 1 }));
        }
        for p in [
            Vector::new(32., 24.),
            Vector::new(15.9, 24.),
            Vector::new(-0.1, 24.),
        ] {
            assert_eq!(policy.select_frame(p), None);
        }
        let sample = sim.sample_world([-40., 24.]).unwrap();
        assert_eq!(sample.local_weight, 0.);
        assert_ne!(sample.background_velocity, [0.; 2]);
        // The request is still validated when permission has left no GPU window.
        let dt = sim.config.dt;
        sim.config.dt *= 2.;
        assert!(sim.try_tick(MoveInput::default()).is_err());
        assert_eq!(sim.steps, 0);
        sim.config.dt = dt;
        sim.policy
            .as_mut()
            .unwrap()
            .lbm_request
            .as_mut()
            .unwrap()
            .outer_band = 0.;
        assert!(sim.try_tick(MoveInput::default()).is_err());
        assert_eq!(sim.elapsed_time, 0.);
        sim.policy
            .as_mut()
            .unwrap()
            .lbm_request
            .as_mut()
            .unwrap()
            .outer_band = 4.;
        sim.try_reset_lbm_profile(crate::GridProfile::Fine).unwrap();
        sim.try_reset().unwrap();
        assert_eq!(
            sim.policy
                .as_ref()
                .unwrap()
                .lbm_request
                .as_ref()
                .unwrap()
                .profile,
            crate::GridProfile::Fine
        );
        assert!(sim.lbm_stats().is_none());
        assert!(!sim.policy_fluid_status().unwrap().window_active);
    }
}
