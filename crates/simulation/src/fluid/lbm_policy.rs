//! CPU preparation of permitted, geometrically usable fixed LBM windows.
use super::{
    lbm_coupling::LbmOptions,
    lbm_geometry::{opening_profiles, rasterize},
};
use crate::{
    circulation::graph::GraphFlow,
    map::{ChunkId, MapDefinition, fluid_policy::BackgroundSource},
};
use blood_fluid_gpu::lbm::{WindowOrigin, WindowSpec};
use rapier2d::prelude::Vector;
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct LbmPolicyRejection {
    pub center: ChunkId,
    pub reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct PreparedLbmPolicy {
    centers: BTreeSet<ChunkId>,
    boundary: Vec<[Vector; 2]>,
    rejections: Vec<LbmPolicyRejection>,
}

impl PreparedLbmPolicy {
    pub fn new(
        map: &MapDefinition,
        peak_flow: &GraphFlow,
        options: &LbmOptions,
    ) -> Result<Self, String> {
        // Validate even when no center is permitted, before trying any geometry.
        if !options.velocity_bound.is_finite()
            || options.velocity_bound <= 0.
            || !options.outer_band.is_finite()
            || options.outer_band <= 0.
            || options.outer_band > 24.
        {
            return Err("invalid LBM velocity bound or outer band".into());
        }
        if map.edges.is_empty()
            || map.pump_edge >= map.edges.len()
            || peak_flow.rates.len() != map.edges.len()
            || peak_flow.rates.iter().any(|v| !v.is_finite())
        {
            return Err("invalid map or peak graph flow".into());
        }
        let mut prepared = Self::default();
        let Some(definition) = &map.fluid else {
            return Ok(prepared);
        };
        for (&center, policy) in &definition.chunks {
            if !policy.allow_lbm {
                continue;
            }
            let check = || -> Result<(), String> {
                for y in -1..=1 {
                    for x in -1..=1 {
                        let chunk = offset(center, x, y)
                            .ok_or("LBM window exceeds chunk coordinate range")?;
                        let policy = definition.policy_at(chunk);
                        if !policy.allow_lbm || policy.background != BackgroundSource::Analytic {
                            return Err("entire 3x3 window requires analytic LBM permission".into());
                        }
                    }
                }
                let spec = WindowSpec {
                    origin: WindowOrigin {
                        block_x: center.x - 1,
                        block_y: center.y - 1,
                    },
                    profile: options.profile,
                };
                if pump_intersects_window(map, spec.world_bounds()) {
                    return Err("LBM window intersects equivalent pump geometry".into());
                }
                // Only one candidate grid lives here; no grid is retained in the policy.
                let raster = rasterize(map, peak_flow, spec)?;
                let (_, profiles) = opening_profiles(map, peak_flow, &raster, spec)?;
                if raster.initial.velocity.iter().any(|u| {
                    let speed = u[0].hypot(u[1]);
                    !speed.is_finite() || speed > options.velocity_bound
                }) {
                    return Err("peak initial speed exceeds LBM velocity bound".into());
                }
                for ((opening, metadata), profile) in raster
                    .geometry
                    .openings
                    .iter()
                    .zip(&raster.openings)
                    .zip(&profiles)
                {
                    // The solver prescribes tangent velocity, not just its normal component.
                    let dot = opening.direction[0] * metadata.normal[0]
                        + opening.direction[1] * metadata.normal[1];
                    if profile.iter().any(|v| {
                        let speed = (v / dot).abs();
                        !speed.is_finite() || speed > options.velocity_bound
                    }) {
                        return Err("peak opening speed exceeds LBM velocity bound".into());
                    }
                }
                Ok(())
            };
            match check() {
                Ok(()) => {
                    prepared.centers.insert(center);
                }
                Err(reason) => prepared
                    .rejections
                    .push(LbmPolicyRejection { center, reason }),
            }
        }
        prepared.boundary = outer_boundary(&prepared.centers);
        Ok(prepared)
    }

    pub fn contains(&self, center: ChunkId) -> bool {
        self.centers.contains(&center)
    }

    pub fn rejections(&self) -> &[LbmPolicyRejection] {
        &self.rejections
    }

    /// Smooth spatial handoff at the union's external edges, including hole edges.
    pub fn activation(&self, player: Vector, band: f32) -> f32 {
        if !player.is_finite()
            || !band.is_finite()
            || band <= 0.
            || !self.contains(ChunkId::at(player.to_array()))
        {
            return 0.;
        }
        let distance = self
            .boundary
            .iter()
            .fold(f32::INFINITY, |distance, [a, b]| {
                let edge = *b - *a;
                let t = ((player - *a).dot(edge) / edge.length_squared()).clamp(0., 1.);
                distance.min((player - (*a + edge * t)).length())
            });
        let t = (distance / band).clamp(0., 1.);
        t * t * (3. - 2. * t)
    }
}

fn offset(chunk: ChunkId, x: i32, y: i32) -> Option<ChunkId> {
    Some(ChunkId {
        x: chunk.x.checked_add(x)?,
        y: chunk.y.checked_add(y)?,
    })
}

fn outer_boundary(centers: &BTreeSet<ChunkId>) -> Vec<[Vector; 2]> {
    let mut boundary = Vec::new();
    for &center in centers {
        let [[x0, y0], [x1, y1]] = center.bounds();
        for (dx, dy, a, b) in [
            (-1, 0, [x0, y0], [x0, y1]),
            (1, 0, [x1, y0], [x1, y1]),
            (0, -1, [x0, y0], [x1, y0]),
            (0, 1, [x0, y1], [x1, y1]),
        ] {
            if !offset(center, dx, dy).is_some_and(|c| centers.contains(&c)) {
                boundary.push([Vector::from(a), Vector::from(b)]);
            }
        }
    }
    boundary
}

/// Conservative capsule/AABB overlap using each segment's radius-expanded AABB.
/// Includes contact with the closed window bounds and crossings with endpoints outside.
fn pump_intersects_window(map: &MapDefinition, [min, max]: [[f32; 2]; 2]) -> bool {
    let pump = &map.edges[map.pump_edge];
    pump.centerline.windows(2).any(|pair| {
        (0..2).all(|axis| {
            pair[0][axis].min(pair[1][axis]) - pump.half_width <= max[axis]
                && pair[0][axis].max(pair[1][axis]) + pump.half_width >= min[axis]
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{
        VesselEdge,
        fluid_policy::{ChunkFluidPolicy, MapFluidDefinition},
    };
    use blood_fluid_gpu::lbm::GridProfile;

    fn fixture() -> (MapDefinition, GraphFlow, LbmOptions) {
        let (mut map, mut flow) = super::super::lbm_geometry::tests::fixture(false, vec![1.]);
        map.edges[0].centerline = vec![[-100., 24.], [100., 24.]];
        map.edges.push(VesselEdge {
            from: 0,
            to: 1,
            half_width: 1.,
            centerline: vec![[200., 200.], [220., 200.]],
        });
        map.pump_edge = 1;
        flow.rates.push(0.);
        map.fluid = Some(MapFluidDefinition {
            geometry_revision: 1,
            max_base_pressure: 1.,
            chunks: ChunkId { x: 1, y: 1 }
                .neighborhood(1)
                .into_iter()
                .map(|c| {
                    (
                        c,
                        ChunkFluidPolicy {
                            background: BackgroundSource::Analytic,
                            allow_lbm: true,
                        },
                    )
                })
                .collect(),
            fields: vec![],
        });
        (
            map,
            flow,
            LbmOptions {
                profile: GridProfile::Coarse,
                ..Default::default()
            },
        )
    }

    #[test]
    fn lbm_policy_requires_all_nine_analytic_permissions_and_checks_options_when_empty() {
        let (mut map, flow, mut options) = fixture();
        let center = ChunkId { x: 1, y: 1 };
        assert!(
            PreparedLbmPolicy::new(&map, &flow, &options)
                .unwrap()
                .contains(center)
        );
        map.fluid
            .as_mut()
            .unwrap()
            .chunks
            .remove(&ChunkId { x: 0, y: 0 });
        let rejected = PreparedLbmPolicy::new(&map, &flow, &options).unwrap();
        assert!(!rejected.contains(center));
        assert!(
            rejected
                .rejections()
                .iter()
                .any(|r| r.center == center && r.reason.contains("3x3"))
        );
        map.fluid.as_mut().unwrap().chunks.insert(
            ChunkId { x: 0, y: 0 },
            ChunkFluidPolicy {
                background: BackgroundSource::Precomputed(crate::map::fluid_policy::FieldId(0)),
                allow_lbm: false,
            },
        );
        assert!(
            !PreparedLbmPolicy::new(&map, &flow, &options)
                .unwrap()
                .contains(center)
        );
        map.fluid.as_mut().unwrap().chunks.clear();
        options.velocity_bound = f32::NAN;
        assert!(PreparedLbmPolicy::new(&map, &flow, &options).is_err());
        options.velocity_bound = 10.;
        options.outer_band = 0.;
        assert!(PreparedLbmPolicy::new(&map, &flow, &options).is_err());
    }

    #[test]
    fn lbm_policy_rejects_real_corner_pump_crossing_and_peak_speed() {
        let (mut map, flow, options) = fixture();
        let center = ChunkId { x: 1, y: 1 };
        map.edges[0].centerline = vec![[-100., -100.], [100., 100.]];
        let rejected = PreparedLbmPolicy::new(&map, &flow, &options).unwrap();
        assert!(
            rejected
                .rejections()
                .iter()
                .any(|r| r.center == center && r.reason.contains("corner"))
        );
        map.edges[0].centerline = vec![[-100., 24.], [100., 24.]];
        // Both endpoints are outside; the entire polyline segment must be considered.
        map.edges[1].centerline = vec![[24., -100.], [24., 100.]];
        let rejected = PreparedLbmPolicy::new(&map, &flow, &options).unwrap();
        assert!(
            rejected
                .rejections()
                .iter()
                .any(|r| r.center == center && r.reason.contains("pump"))
        );
        // The tube radius touches the closed window although its centerline is outside.
        map.edges[1].centerline = vec![[-100., -1.], [100., -1.]];
        assert!(
            !PreparedLbmPolicy::new(&map, &flow, &options)
                .unwrap()
                .contains(center)
        );
        map.edges[1].centerline = vec![[200., 200.], [220., 200.]];
        let slow = LbmOptions {
            velocity_bound: 0.1,
            ..options
        };
        let rejected = PreparedLbmPolicy::new(&map, &flow, &slow).unwrap();
        assert!(
            rejected
                .rejections()
                .iter()
                .any(|r| r.center == center && r.reason.contains("speed"))
        );
    }

    #[test]
    fn lbm_policy_activation_skips_shared_edges_and_handles_negative_coordinates_and_holes() {
        let (mut map, flow, options) = fixture();
        let definition = map.fluid.as_mut().unwrap();
        for y in 0..=2 {
            definition.chunks.insert(
                ChunkId { x: 3, y },
                ChunkFluidPolicy {
                    background: BackgroundSource::Analytic,
                    allow_lbm: true,
                },
            );
        }
        let policy = PreparedLbmPolicy::new(&map, &flow, &options).unwrap();
        assert!(policy.contains(ChunkId { x: 2, y: 1 }));
        for x in [31.9, 32., 32.1] {
            assert_eq!(policy.activation(Vector::new(x, 24.), 4.), 1.);
        }
        assert_eq!(policy.activation(Vector::new(16., 24.), 4.), 0.);
        assert_eq!(policy.activation(Vector::new(18., 24.), 4.), 0.5);
        assert_eq!(policy.activation(Vector::new(48., 24.), 4.), 0.);
        // Exercise the same production boundary construction for a negative-coordinate ring.
        let centers: BTreeSet<_> = ChunkId { x: -2, y: -2 }
            .neighborhood(1)
            .into_iter()
            .filter(|c| *c != ChunkId { x: -2, y: -2 })
            .collect();
        let ring = PreparedLbmPolicy {
            boundary: outer_boundary(&centers),
            centers,
            rejections: vec![],
        };
        assert_eq!(
            ring.activation(Vector::new(-32.1, -24.), 4.),
            ring.activation(Vector::new(-31.9, -32.1), 4.)
        );
        assert_eq!(ring.activation(Vector::new(-34., -24.), 4.), 0.5);
        assert_eq!(ring.activation(Vector::new(-24., -24.), 4.), 0.);
        assert_eq!(ring.activation(Vector::new(-48., -24.), 4.), 0.);
        assert_eq!(ring.activation(Vector::new(-46., -24.), 4.), 0.5);
        assert_eq!(ring.activation(Vector::new(-40., -32.), 4.), 1.);
        assert_eq!(ring.activation(Vector::new(-48.1, -24.), 4.), 0.);
    }
}
