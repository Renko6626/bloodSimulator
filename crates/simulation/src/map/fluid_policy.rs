//! Persistent background-source labels and optional precomputed field assets.

use super::ChunkId;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundSource {
    Analytic,
    Precomputed(FieldId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkFluidPolicy {
    pub background: BackgroundSource,
    pub allow_lbm: bool,
}

#[derive(Clone, Debug)]
pub struct MapFluidDefinition {
    pub geometry_revision: u64,
    pub max_base_pressure: f32,
    pub chunks: BTreeMap<ChunkId, ChunkFluidPolicy>,
    pub fields: Vec<PrecomputedField>,
}

impl MapFluidDefinition {
    pub fn policy_at(&self, chunk: ChunkId) -> ChunkFluidPolicy {
        self.chunks
            .get(&chunk)
            .copied()
            .unwrap_or(ChunkFluidPolicy {
                background: BackgroundSource::Analytic,
                allow_lbm: false,
            })
    }
}

/// Row-major samples of a fixed-viscosity field, scaled by a common nonnegative factor.
#[derive(Clone, Debug)]
pub struct PrecomputedField {
    pub geometry_revision: u64,
    pub viscosity: f32,
    pub reference_pressure: f32,
    pub max_scale: f32,
    /// World position of the first sample; asset edges must cover interpolation support.
    pub origin: [f32; 2],
    pub spacing: f32,
    pub size: [usize; 2],
    pub velocity: Vec<[f32; 2]>,
    pub fluid: Vec<bool>,
    pub ports: Vec<FieldPort>,
}

/// A vessel cross-section defined by its position, tangent, and half-width at `along`.
#[derive(Clone, Debug)]
pub struct FieldPort {
    pub edge: usize,
    pub along: f32,
    /// Either -1 or +1 relative to the vessel tangent.
    pub outward_sign: f32,
    /// Signed outward two-dimensional flux (unit depth).
    pub reference_flux: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_lookup_uses_world_chunks_and_keeps_unconfigured_neighbors_closed() {
        let chunk = ChunkId::at([-0.1, 16.0]);
        assert_eq!(chunk, ChunkId { x: -1, y: 1 });
        let policy = ChunkFluidPolicy {
            background: BackgroundSource::Precomputed(FieldId(0)),
            allow_lbm: false,
        };
        let definition = MapFluidDefinition {
            geometry_revision: 1,
            max_base_pressure: 1.0,
            chunks: BTreeMap::from([(chunk, policy)]),
            fields: vec![],
        };

        assert_eq!(definition.policy_at(chunk), policy);
        assert_eq!(
            definition.policy_at(ChunkId::at([0.1, 16.0])),
            ChunkFluidPolicy {
                background: BackgroundSource::Analytic,
                allow_lbm: false,
            }
        );
    }
}
