//! Heart-background cuts and source labels. No GPU or solver ownership here.
use super::{ChunkId, MapDefinition, fluid_policy::*};
use rapier2d::prelude::Vector;

pub const DOMAIN_ORIGIN: [f32; 2] = [464., 656.];
pub const DOMAIN_CELLS: u32 = 384;
pub const SPACING: f32 = 0.5;
pub const SOURCE_MIN: [f32; 2] = [464., 656.];
pub const SOURCE_MAX: [f32; 2] = [608., 848.];
pub const REFERENCE_RATE: f32 = 48.;

pub fn ports(map: &MapDefinition, rates: &[f32]) -> Result<Vec<FieldPort>, String> {
    if rates.len() != map.edges.len() {
        return Err("heart ports need a reference rate for every edge".into());
    }
    [488., 528., 562., 600.]
        .into_iter()
        .map(|x| {
            let point = Vector::new(x, SOURCE_MIN[1]);
            let edge = map.nearest_edge(point);
            let projection = map.project_edge(edge, point);
            if projection.distance > 1e-3 {
                return Err(format!("heart cut is not on its guide: {point:?}"));
            }
            let outward_sign = -projection.tangent.y.signum();
            Ok(FieldPort {
                edge,
                along: projection.along,
                outward_sign,
                reference_flux: rates[edge] * outward_sign,
            })
        })
        .collect()
}

pub fn definition(field: PrecomputedField) -> MapFluidDefinition {
    let min = ChunkId::at(SOURCE_MIN);
    let max = ChunkId::at(SOURCE_MAX);
    MapFluidDefinition {
        geometry_revision: 1,
        max_base_pressure: field.reference_pressure,
        chunks: (min.y..max.y)
            .flat_map(|y| {
                (min.x..max.x).map(move |x| {
                    (
                        ChunkId { x, y },
                        ChunkFluidPolicy {
                            background: BackgroundSource::Precomputed(FieldId(0)),
                            allow_lbm: false,
                        },
                    )
                })
            })
            .collect(),
        fields: vec![field],
    }
}
