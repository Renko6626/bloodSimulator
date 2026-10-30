//! Conversion only: normal map/background validation remains required on import.
use crate::map::{
    fluid_asset::validate_structure,
    fluid_policy::{FieldPort, PrecomputedField},
};
use blood_fluid_gpu::lbm::bake::BakedFlow;

pub fn to_precomputed_field(
    baked: &BakedFlow,
    geometry_revision: u64,
    reference_pressure: f32,
    max_scale: f32,
    ports: Vec<FieldPort>,
) -> Result<PrecomputedField, String> {
    baked.domain.validate()?;
    let side = usize::try_from(baked.domain.cells_per_side)
        .map_err(|_| "baked dimension exceeds usize")?;
    let field = PrecomputedField {
        geometry_revision,
        viscosity: baked.viscosity,
        reference_pressure,
        max_scale,
        origin: baked.domain.origin.map(|x| x + baked.domain.spacing * 0.5),
        spacing: baked.domain.spacing,
        size: [side; 2],
        velocity: baked.velocity.clone(),
        fluid: baked.fluid.clone(),
        ports,
    };
    validate_structure(&field)?;
    Ok(field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use blood_fluid_gpu::lbm::{
        Diagnostics, DomainSpec,
        bake::{BakeReport, BakeSchedule},
    };
    #[test]
    fn conversion_preserves_centers_row_order_mask_and_solver_viscosity() {
        let baked = BakedFlow {
            domain: DomainSpec {
                origin: [-1., 3.],
                cells_per_side: 2,
                spacing: 0.5,
            },
            viscosity: 0.125,
            velocity: vec![[1., 2.], [0., 0.], [3., 4.], [5., 6.]],
            fluid: vec![true, false, true, true],
            report: BakeReport {
                schedule: BakeSchedule {
                    steps_per_frame: 1,
                    warmup_frames: 0,
                    sample_frames: 2,
                    sample_stride: 1,
                },
                lattice_dt: 0.01,
                samples: 2,
                warmup_simulated_seconds: 0.,
                sample_simulated_seconds: 0.02,
                wall_seconds: 0.,
                density_min: 1.,
                density_max: 1.,
                mean_opening_fluxes: vec![],
                half_mean_velocity_rms: 0.,
                half_mean_velocity_max: 0.,
                final_diagnostics: Diagnostics {
                    density_min: 1.,
                    density_max: 1.,
                    max_speed: 1.,
                    mass: 3.,
                    opening_fluxes: vec![],
                    software_adapter: false,
                    adapter_name: String::new(),
                },
            },
        };
        let field = to_precomputed_field(&baked, 7, 2., 3., vec![]).unwrap();
        assert_eq!(field.origin, [-0.75, 3.25]);
        assert_eq!(field.spacing, 0.5);
        assert_eq!(field.size, [2, 2]);
        assert_eq!(field.velocity, baked.velocity);
        assert_eq!(field.fluid, baked.fluid);
        assert_eq!(field.viscosity, 0.125);
        assert_eq!(
            (
                field.geometry_revision,
                field.reference_pressure,
                field.max_scale
            ),
            (7, 2., 3.)
        );
    }
}
