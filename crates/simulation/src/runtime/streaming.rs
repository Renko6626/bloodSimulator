use super::*;
use crate::{
    StreamingSnapshot, WorldChunkDelta,
    map::{CHUNK_SIZE, ChunkId, SCENE_RADIUS},
    world_streaming::{LoadedChunk, WorldStreaming},
};
use rapier2d::parry::{query::PointQuery, shape::Segment};

pub(super) fn scene_center(position: Vector) -> Vector {
    let [min, max] = ChunkId::at(position.to_array()).scene_bounds();
    (Vector::from_array(min) + Vector::from_array(max)) * 0.5
}

pub(super) fn local_config(mut config: Config) -> Config {
    let half = CHUNK_SIZE * (SCENE_RADIUS as f32 + 0.5);
    config.cell_mode = CellSimulationMode::Local(crate::CellPopulationOptions {
        activation_half_extent: half,
        retention_half_extent: half,
        protected_half_extent: 0.,
        ..crate::CellPopulationOptions::default()
    });
    config
}

pub(super) fn in_view(
    stream: &WorldStreaming,
    robot: Vector,
    position: Vector,
    radius: f32,
) -> bool {
    let [min, max] = stream.view.unwrap_or([
        (robot - Vector::splat(CHUNK_SIZE)).to_array(),
        (robot + Vector::splat(CHUNK_SIZE)).to_array(),
    ]);
    position.x + radius >= min[0]
        && position.y + radius >= min[1]
        && position.x - radius <= max[0]
        && position.y - radius <= max[1]
}

fn cover_bounds(min: Vector, max: Vector) -> impl Iterator<Item = ChunkId> {
    let a = ChunkId::at(min.to_array());
    let b = ChunkId::at([max.x.next_down(), max.y.next_down()]);
    (a.y..=b.y).flat_map(move |y| (a.x..=b.x).map(move |x| ChunkId { x, y }))
}

impl Simulation {
    pub fn new_streamed_world(config: Config, map: MapDefinition) -> Self {
        assert!(
            map.fluid.is_none(),
            "policy maps require try_new_policy_world"
        );
        let config = local_config(config);
        let network = GraphNetwork::new(&map, config.viscosity);
        let flow = network.solve(config.pressure);
        let center = ChunkId::at(map.spawn);
        let mut sim = Self::build(
            config,
            Environment::World {
                map,
                geometry: None,
                background: None,
                network,
                flow,
            },
        );
        sim.streaming = Some(WorldStreaming::new(center));
        sim.update_streaming();
        sim.seed_white_cells();
        sim
    }

    pub(super) fn enable_streaming(&mut self) {
        let center = ChunkId::at(self.physics.bodies[self.robot].translation().to_array());
        self.streaming = Some(WorldStreaming::new(center));
        self.update_streaming();
    }

    #[cfg(feature = "lbm-gpu")]
    pub fn new_streamed_lbm_world(
        config: Config,
        options: crate::LbmOptions,
        map: MapDefinition,
    ) -> Result<Self, String> {
        let mut sim = Self::new_lbm_world_seeded(config, options, map, None, true)?;
        sim.seed_white_cells();
        Ok(sim)
    }

    #[cfg(feature = "lbm-gpu")]
    pub fn new_streamed_lbm_validation(
        config: Config,
        options: crate::LbmOptions,
    ) -> Result<Self, String> {
        let map = crate::lbm_validation::map();
        let centers = crate::lbm_validation::centers(&map, &config);
        let mut sim = Self::new_lbm_world_seeded(config, options, map, Some(centers), true)?;
        sim.validation_scene = true;
        Ok(sim)
    }

    pub(super) fn update_streaming(&mut self) {
        let Some(mut stream) = self.streaming.take() else {
            return;
        };
        let Environment::World { map, .. } = &self.environment else {
            unreachable!()
        };
        let robot = self.physics.bodies[self.robot].translation();
        stream.center = ChunkId::at(robot.to_array());
        let scene = stream.center.neighborhood(SCENE_RADIUS);
        let [min, max] = stream.center.scene_bounds().map(Vector::from_array);
        for cell in &mut self.cells {
            if cell.unload_policy != cell::UnloadPolicy::Hibernate {
                continue;
            }
            if let Some(handle) = cell.active_body() {
                let (a, b) = cell_bounds(&self.physics.soft_bodies[handle]);
                if b.x < min.x || b.y < min.y || a.x > max.x || a.y > max.y {
                    cell.hibernate(&mut self.physics);
                }
            }
        }
        let mut required = scene.clone();
        for cell in &self.cells {
            let bounds = if let Some(handle) = cell.active_body() {
                Some(cell_bounds(&self.physics.soft_bodies[handle]))
            } else if let cell::CellBody::Dormant(body) = &cell.body {
                scene
                    .contains(&ChunkId::at(body.center().to_array()))
                    .then(|| point_bounds(&body.positions))
            } else {
                None
            };
            if let Some((a, b)) = bounds {
                // Existing membrane/wall radii; keep adjacent wall ownership at seams.
                required.extend(cover_bounds(
                    a - Vector::splat(0.15),
                    b + Vector::splat(0.15),
                ));
            }
        }
        let mut changed = false;
        stream.chunks.retain(|id, chunk| {
            if required.contains(id) {
                return true;
            }
            for handle in &chunk.walls {
                self.physics.remove_collider(*handle);
            }
            changed = true;
            false
        });
        for id in required {
            if stream.chunks.contains_key(&id) {
                continue;
            }
            let geometry = map.build_chunk_geometry(id);
            let walls = geometry
                .core
                .walls
                .iter()
                .flat_map(|c| c.windows(2))
                .filter(|p| p[0] != p[1])
                .map(|p| {
                    self.physics.insert_collider(
                        ColliderBuilder::capsule_from_endpoints(
                            Vector::from_array(p[0]),
                            Vector::from_array(p[1]),
                            0.08,
                        )
                        .friction(0.05),
                        None,
                    )
                })
                .collect();
            stream.chunks.insert(id, LoadedChunk { geometry, walls });
            changed = true;
        }
        if changed {
            self.physics.detect_collisions(&(), &());
            self.walls = stream
                .chunks
                .values()
                .flat_map(|c| c.geometry.core.walls.iter().cloned())
                .collect();
        }
        for index in 0..self.cells.len() {
            let cell::CellBody::Dormant(body) = &self.cells[index].body else {
                continue;
            };
            let center = body.center();
            let radius = body
                .positions
                .iter()
                .map(|p| p.distance(center))
                .fold(0., f32::max);
            if !scene.contains(&ChunkId::at(center.to_array()))
                || in_view(&stream, robot, center, radius)
            {
                continue;
            }
            if self.can_restore(body) {
                self.cells[index].restore(&mut self.physics, &self.config);
            }
        }
        self.streaming = Some(stream);
    }

    fn can_restore(&self, saved: &cell::DormantBody) -> bool {
        let map = self.world_map().unwrap();
        if saved.positions.iter().any(|p| map.signed_distance(*p) > 0.) {
            return false;
        }
        let robot = self.physics.bodies[self.robot].translation();
        if rapier2d::parry::utils::point_in_poly2d(robot, &saved.positions) {
            return false;
        }
        for i in 0..saved.positions.len() {
            let segment = Segment::new(
                saved.positions[i],
                saved.positions[(i + 1) % saved.positions.len()],
            );
            if segment
                .project_local_point(robot, true)
                .point
                .distance(robot)
                < 0.85 + 0.07
            {
                return false;
            }
        }
        let (a, b) = point_bounds(&saved.positions);
        for cell in &self.cells {
            let Some(handle) = cell.active_body() else {
                continue;
            };
            let body = &self.physics.soft_bodies[handle];
            let (c, d) = cell_bounds(body);
            if b.x < c.x || b.y < c.y || a.x > d.x || a.y > d.y {
                continue;
            }
            let points: Vec<_> = body.particle_positions().collect();
            if rapier2d::parry::utils::point_in_poly2d(points[0], &saved.positions)
                || rapier2d::parry::utils::point_in_poly2d(saved.positions[0], &points)
            {
                return false;
            }
            for i in 0..saved.positions.len() {
                let s = Segment::new(
                    saved.positions[i],
                    saved.positions[(i + 1) % saved.positions.len()],
                );
                for j in 0..points.len() {
                    let t = Segment::new(points[j], points[(j + 1) % points.len()]);
                    if rapier2d::parry::query::intersection_test(
                        &Pose::IDENTITY,
                        &s,
                        &Pose::IDENTITY,
                        &t,
                    )
                    .is_ok_and(|hit| hit.intersecting)
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn sync_world_view(
        &mut self,
        bounds: [[f32; 2]; 2],
        show_flow: bool,
    ) -> Result<WorldChunkDelta, String> {
        if !bounds.iter().flatten().all(|v| v.is_finite())
            || bounds[1][0] <= bounds[0][0]
            || bounds[1][1] <= bounds[0][1]
        {
            return Err("invalid camera bounds".into());
        }
        let stream = self
            .streaming
            .as_mut()
            .ok_or("world streaming is not enabled")?;
        stream.view = Some(bounds);
        stream.show_flow = show_flow;
        let desired = stream.center.neighborhood(SCENE_RADIUS);
        let added = desired
            .difference(&stream.sent)
            .filter_map(|id| stream.chunks.get(id).map(|c| c.geometry.clone()))
            .collect();
        let removed = stream.sent.difference(&desired).copied().collect();
        stream.sent = desired;
        Ok(WorldChunkDelta {
            reset: std::mem::take(&mut stream.reset_pending),
            added,
            removed,
        })
    }

    pub(super) fn streaming_snapshot(&self) -> Option<StreamingSnapshot> {
        self.streaming.as_ref().map(|s| StreamingSnapshot {
            center: [s.center.x, s.center.y],
            loaded_chunks: s.chunks.len(),
            wall_colliders: s.chunks.values().map(|c| c.walls.len()).sum(),
            active_cells: self
                .cells
                .iter()
                .filter(|c| c.active_body().is_some())
                .count(),
            dormant_cells: self
                .cells
                .iter()
                .filter(|c| c.active_body().is_none())
                .count(),
        })
    }
}

fn point_bounds(points: &[Vector]) -> (Vector, Vector) {
    points.iter().fold(
        (
            Vector::splat(f32::INFINITY),
            Vector::splat(f32::NEG_INFINITY),
        ),
        |(a, b), p| (a.min(*p), b.max(*p)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streamed_world_preserves_identity_and_releases_distant_resources() {
        let config = Config {
            cell_count: 0,
            pressure: 0.,
            white_cells: crate::WhiteCellOptions {
                count: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut sim = Simulation::new_streamed_world(config, MapDefinition::small_world());
        assert!(sim.world_geometry().is_none());
        let index = sim.insert_white_cell(Vector::new(0., -12.));
        sim.cells[index].damage(50.);
        let id = sim.cells[index].id;
        let handle = sim.cells[index].active_body().unwrap();
        let original: Vec<_> = sim.physics.soft_bodies[handle]
            .particle_positions()
            .collect();
        sim.physics.bodies[sim.robot].set_translation(Vector::new(80., 0.), true);
        sim.update_cell_population();
        sim.update_streaming();
        assert!(sim.physics.soft_bodies.get(handle).is_none());
        assert_eq!(sim.streaming_snapshot().unwrap().dormant_cells, 1);
        assert_eq!(sim.snapshot().immunity.white_bodies, 1);
        assert!(sim.snapshot().cells.is_empty());
        sim.physics.bodies[sim.robot].set_translation(Vector::new(-20., 0.), true);
        sim.update_cell_population();
        sim.update_streaming();
        let handle = sim.cells[index].active_body().unwrap();
        assert_eq!(sim.cells[index].id, id);
        assert_eq!(sim.cells[index].health, 100.);
        assert_eq!(
            sim.physics.soft_bodies[handle]
                .particle_positions()
                .collect::<Vec<_>>(),
            original
        );
        assert_eq!(sim.streaming_snapshot().unwrap().loaded_chunks, 25);
        let walls = sim.physics.colliders.len();
        for x in [160., 320., 160., -20.] {
            sim.physics.bodies[sim.robot].set_translation(Vector::new(x, 0.), true);
            sim.update_cell_population();
            sim.update_streaming();
        }
        assert_eq!(sim.physics.colliders.len(), walls);
        sim.set_paused(true);
        sim.reset();
        assert!(sim.streaming.is_some());
        assert!(sim.snapshot().paused);
        assert!(sim.world_geometry().is_none());
    }

    #[test]
    fn dormant_ai_freezes_but_corpse_lifetime_still_expires() {
        let config = Config {
            cell_count: 0,
            pressure: 0.,
            white_cells: crate::WhiteCellOptions {
                count: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut sim = Simulation::new_streamed_world(config, MapDefinition::small_world());
        let index = sim.insert_white_cell(Vector::new(0., -12.));
        sim.cells[index].white.as_mut().unwrap().memory = 3.;
        sim.physics.bodies[sim.robot].set_translation(Vector::new(80., 0.), true);
        sim.tick(MoveInput::default());
        for _ in 0..5 {
            sim.tick(MoveInput::default());
        }
        assert_eq!(sim.cells[index].white.as_ref().unwrap().memory, 3.);
        sim.cells[index].damage(1000.);
        sim.set_paused(true);
        sim.tick(MoveInput::default());
        assert_eq!(sim.cells[index].dead_age, Some(0.));
        sim.single_step(MoveInput::default());
        assert!(sim.cells[index].dead_age.unwrap() > 0.);
        sim.set_paused(false);
        for _ in 0..365 {
            sim.tick(MoveInput::default());
        }
        assert_eq!(sim.snapshot().immunity.white_bodies, 0);
        assert!(sim.physics.soft_bodies.is_empty());
    }
}
