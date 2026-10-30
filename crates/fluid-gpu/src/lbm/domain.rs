//! Square world-space domain, independent of realtime window profiles.
use super::LbmResult;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DomainSpec {
    /// Lower-left cell edge in world units (values are stored at cell centers).
    pub origin: [f32; 2],
    pub cells_per_side: u32,
    pub spacing: f32,
}

impl DomainSpec {
    pub fn validate(self) -> LbmResult<()> {
        if self.cells_per_side == 0
            || !self.spacing.is_finite()
            || self.spacing <= 0.
            || self.origin.iter().any(|x| !x.is_finite())
        {
            return Err("domain requires finite origin, positive spacing and cell count".into());
        }
        // Shader population indices and dispatch cell counts use u32.
        let cells = u64::from(self.cells_per_side).pow(2);
        if cells > u64::from(u32::MAX) / 9 || cells * 9 * 4 > isize::MAX as u64 {
            return Err("domain cell count exceeds population indexing/allocation range".into());
        }
        let end = self.world_bounds()[1];
        if end
            .iter()
            .zip(self.origin)
            .any(|(&hi, lo)| !hi.is_finite() || hi <= lo || lo + self.spacing <= lo)
        {
            return Err("domain world bounds are not finite or spacing is unrepresentable".into());
        }
        Ok(())
    }

    pub fn world_bounds(self) -> [[f32; 2]; 2] {
        let extent = self.cells_per_side as f32 * self.spacing;
        [self.origin, self.origin.map(|x| x + extent)]
    }
}
