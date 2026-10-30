//! Loaded scene resources on the fixed world grid. Simulation owns transitions.
use crate::map::{ChunkGeometry, ChunkId};
use rapier2d::prelude::ColliderHandle;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct LoadedChunk {
    pub geometry: ChunkGeometry,
    pub walls: Vec<ColliderHandle>,
}

pub(crate) struct WorldStreaming {
    pub center: ChunkId,
    pub chunks: BTreeMap<ChunkId, LoadedChunk>,
    pub sent: BTreeSet<ChunkId>,
    pub reset_pending: bool,
    pub view: Option<[[f32; 2]; 2]>,
    pub show_flow: bool,
}

impl WorldStreaming {
    pub fn new(center: ChunkId) -> Self {
        Self {
            center,
            chunks: BTreeMap::new(),
            sent: BTreeSet::new(),
            reset_pending: true,
            view: None,
            show_flow: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct WorldChunkDelta {
    pub reset: bool,
    pub added: Vec<ChunkGeometry>,
    pub removed: Vec<ChunkId>,
}

#[derive(Clone, Debug)]
pub struct StreamingSnapshot {
    pub center: [i32; 2],
    pub loaded_chunks: usize,
    pub wall_colliders: usize,
    pub active_cells: usize,
    pub dormant_cells: usize,
}
