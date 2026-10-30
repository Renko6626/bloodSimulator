//! Plain world-space contract for a block-aligned LBM window.

const BLOCK_SIZE: i64 = blood_fluid::WORLD_CHUNK_SIZE as i64;
const WINDOW_BLOCKS: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridProfile {
    Fine,
    Coarse,
}

impl GridProfile {
    pub const fn cells_per_unit(self) -> u32 {
        match self {
            Self::Fine => 4,
            Self::Coarse => 2,
        }
    }

    pub const fn cells_per_block(self) -> u32 {
        self.cells_per_unit() * BLOCK_SIZE as u32
    }

    pub const fn cells_per_side(self) -> u32 {
        self.cells_per_block() * WINDOW_BLOCKS
    }

    /// World units per lattice cell.
    pub fn cell_spacing(self) -> f32 {
        1.0 / self.cells_per_unit() as f32
    }
}

/// The window's lower-left logical block coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowOrigin {
    pub block_x: i32,
    pub block_y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowSpec {
    pub origin: WindowOrigin,
    pub profile: GridProfile,
}

impl WindowSpec {
    /// Lower-left and upper-right bounds in world units.
    pub fn world_bounds(self) -> [[f32; 2]; 2] {
        let x = self.origin.block_x as f32 * BLOCK_SIZE as f32;
        let y = self.origin.block_y as f32 * BLOCK_SIZE as f32;
        let size = (BLOCK_SIZE * WINDOW_BLOCKS as i64) as f32;
        [[x, y], [x + size, y + size]]
    }

    /// Origin displacement in this window's lattice cells; profiles must match.
    pub fn origin_cell_offset_from(self, previous: Self) -> [i64; 2] {
        assert_eq!(self.profile, previous.profile);
        let cells = i64::from(self.profile.cells_per_block());
        [
            (i64::from(self.origin.block_x) - i64::from(previous.origin.block_x)) * cells,
            (i64::from(self.origin.block_y) - i64::from(previous.origin.block_y)) * cells,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowSide {
    Left,
    Right,
    Bottom,
    Top,
}

/// Opening cells are a half-open range along the side, in increasing x or y.
#[derive(Clone, Debug, PartialEq)]
pub struct OpeningSpec {
    pub side: WindowSide,
    pub cells: std::ops::Range<u32>,
    /// World-space unit vector pointing along the vessel segment.
    pub direction: [f32; 2],
    /// World units squared per game time; positive outward from the window.
    pub outward_flow: f32,
    /// Background speed in world units per game time.
    pub background_speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalSampleStatus {
    Valid,
    OutsideWindow,
    SolidOrInvalid,
}

/// Velocities are in world units per game time. The simulation supplies the
/// background and weight; an outside fluid point uses weight zero. Solid or
/// invalid positions must not receive exterior background flow.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidSample {
    pub local_velocity: [f32; 2],
    pub background_velocity: [f32; 2],
    pub local_weight: f32,
    pub status: LocalSampleStatus,
}

mod gpu;
mod model;
mod stencil;
pub use gpu::*;

#[cfg(test)]
mod tests;
