//! Two precomputed, overlapping local domains sharing the caller's physics world.
use super::{FluidStats, LocalFluid, ORIGIN};
use crate::{circulation::graph::GraphFlow, map::MapDefinition};
use rapier2d::prelude::*;
use std::time::Instant;

pub(crate) struct LocalRegions {
    regions: [LocalFluid; 2],
    active: usize,
}

impl LocalRegions {
    pub fn new(
        map: &MapDefinition,
        flow: &GraphFlow,
        viscosity: f32,
        cells: usize,
    ) -> Result<Self, String> {
        Ok(Self {
            regions: [
                LocalFluid::new_at(map, flow, viscosity, cells, ORIGIN)?,
                LocalFluid::new_at(map, flow, viscosity, cells, [-18.0, -16.0])?,
            ],
            active: 0,
        })
    }

    pub fn begin_frame(&mut self) {
        for region in &mut self.regions {
            region.begin_frame();
        }
    }

    pub fn prepare_pressure(&mut self, pressure: f32) {
        for region in &mut self.regions {
            region.prepare_pressure(pressure);
        }
    }

    /// Rebuild the incoming field from existing forces without touching bodies.
    /// Only advance() applies the one local reaction impulse for this substep.
    pub fn prepare_step(
        &mut self,
        world: &PhysicsWorld,
        cells: &[SoftBodyHandle],
        robot: RigidBodyHandle,
        drag: f32,
        radius: f32,
    ) -> Result<(), String> {
        let next = 1 - self.active;
        let position = world.bodies[robot].translation();
        if self.regions[next].clearance(position)
            <= self.regions[self.active].clearance(position) + 1.0
        {
            return Ok(());
        }
        let started = Instant::now();
        let (left, right) = self.regions.split_at_mut(1);
        let (outgoing, incoming) = if self.active == 0 {
            (&left[0], &mut right[0])
        } else {
            (&right[0], &mut left[0])
        };
        let previous_velocity = outgoing.sample(position);
        incoming.history.clone_from(&outgoing.history);
        incoming.prime_from_history(world, cells, robot, drag, radius)?;
        let bounds = incoming.stats.bounds;
        incoming.stats = outgoing.stats.clone();
        incoming.stats.bounds = bounds;
        let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
        incoming.stats.solve_ms += elapsed_ms;
        incoming.stats.region_switches += 1;
        incoming.stats.handoff_ms += elapsed_ms;
        incoming.stats.handoff_velocity_delta = incoming
            .stats
            .handoff_velocity_delta
            .max((incoming.sample(position) - previous_velocity).length());
        self.active = next;
        Ok(())
    }

    pub fn weight(&self, position: Vector, radius: f32) -> f32 {
        self.regions[self.active].weight(position, radius)
    }

    pub fn sample(&self, position: Vector) -> Vector {
        self.regions[self.active].sample(position)
    }

    pub fn positions(&self) -> impl Iterator<Item = Vector> + '_ {
        self.regions[self.active].positions()
    }

    pub fn stats(&self) -> &FluidStats {
        &self.regions[self.active].stats
    }

    pub fn advance(
        &mut self,
        world: &mut PhysicsWorld,
        cells: &[SoftBodyHandle],
        robot: RigidBodyHandle,
        dt: f32,
        drag: f32,
        radius: f32,
    ) -> Result<(), String> {
        self.regions[self.active].advance(world, cells, robot, dt, drag, radius)
    }
}
