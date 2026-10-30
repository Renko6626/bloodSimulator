//! Versioned binary storage for the existing field model. Map semantics are checked
//! separately by normal background preparation after loading.
use super::fluid_policy::{FieldPort, PrecomputedField};
use std::io::{Read, Write};
const MAGIC: &[u8; 8] = b"BFLOW001";
const HEADER_BYTES: usize = 52;

pub fn write_field(mut writer: impl Write, field: &PrecomputedField) -> Result<(), String> {
    validate_structure(field)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&field.geometry_revision.to_le_bytes());
    for value in [
        field.viscosity,
        field.reference_pressure,
        field.max_scale,
        field.origin[0],
        field.origin[1],
        field.spacing,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for value in [field.size[0], field.size[1], field.ports.len()] {
        bytes.extend_from_slice(
            &u32::try_from(value)
                .map_err(|_| "field size/port count exceeds u32")?
                .to_le_bytes(),
        );
    }
    for port in &field.ports {
        bytes.extend_from_slice(
            &u32::try_from(port.edge)
                .map_err(|_| "field edge exceeds u32")?
                .to_le_bytes(),
        );
        for value in [port.along, port.outward_sign, port.reference_flux] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    for velocity in &field.velocity {
        for value in velocity {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes.extend(field.fluid.iter().map(|&fluid| u8::from(fluid)));
    writer.write_all(&bytes).map_err(|e| e.to_string())
}

pub fn read_field(mut reader: impl Read) -> Result<PrecomputedField, String> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    let mut input = bytes.as_slice();
    if take::<8>(&mut input)? != *MAGIC {
        return Err("unsupported flow asset magic/version".into());
    }
    let geometry_revision = u64::from_le_bytes(take(&mut input)?);
    let viscosity = read_f32(&mut input)?;
    let reference_pressure = read_f32(&mut input)?;
    let max_scale = read_f32(&mut input)?;
    let origin = [read_f32(&mut input)?, read_f32(&mut input)?];
    let spacing = read_f32(&mut input)?;
    let size = [read_usize(&mut input)?, read_usize(&mut input)?];
    let port_count = read_usize(&mut input)?;
    let count = size[0]
        .checked_mul(size[1])
        .ok_or("field dimensions overflow")?;
    let expected = count
        .checked_mul(9)
        .and_then(|n| port_count.checked_mul(16).and_then(|p| n.checked_add(p)))
        .ok_or("field payload size overflow")?;
    if input.len() != expected {
        return Err("field payload length does not match dimensions/ports".into());
    }
    let mut ports = Vec::with_capacity(port_count);
    for _ in 0..port_count {
        ports.push(FieldPort {
            edge: read_usize(&mut input)?,
            along: read_f32(&mut input)?,
            outward_sign: read_f32(&mut input)?,
            reference_flux: read_f32(&mut input)?,
        });
    }
    let mut velocity = Vec::with_capacity(count);
    for _ in 0..count {
        velocity.push([read_f32(&mut input)?, read_f32(&mut input)?]);
    }
    let fluid = input
        .iter()
        .map(|&byte| match byte {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("invalid field mask byte".to_string()),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let field = PrecomputedField {
        geometry_revision,
        viscosity,
        reference_pressure,
        max_scale,
        origin,
        spacing,
        size,
        velocity,
        fluid,
        ports,
    };
    validate_structure(&field)?;
    Ok(field)
}
fn take<const N: usize>(input: &mut &[u8]) -> Result<[u8; N], String> {
    let value = input
        .get(..N)
        .ok_or("truncated flow asset")?
        .try_into()
        .unwrap();
    *input = &input[N..];
    Ok(value)
}
fn read_f32(input: &mut &[u8]) -> Result<f32, String> {
    Ok(f32::from_le_bytes(take(input)?))
}
fn read_usize(input: &mut &[u8]) -> Result<usize, String> {
    usize::try_from(u32::from_le_bytes(take(input)?))
        .map_err(|_| "field integer exceeds usize".into())
}

pub(crate) fn validate_structure(field: &PrecomputedField) -> Result<(), String> {
    let count = field.size[0]
        .checked_mul(field.size[1])
        .ok_or("field dimensions overflow")?;
    if field
        .size
        .iter()
        .any(|&n| n < 2 || u32::try_from(n).is_err())
        || count != field.velocity.len()
        || count != field.fluid.len()
        || u32::try_from(field.ports.len()).is_err()
    {
        return Err("invalid field dimensions or array lengths".into());
    }
    if [field.viscosity, field.reference_pressure, field.spacing]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.)
        || !field.max_scale.is_finite()
        || field.max_scale < 0.
        || field
            .origin
            .iter()
            .chain(field.velocity.iter().flatten())
            .any(|v| !v.is_finite())
    {
        return Err("invalid field numeric values".into());
    }
    for k in 0..2 {
        let end = field.origin[k] + (field.size[k] - 1) as f32 * field.spacing;
        if !end.is_finite()
            || end <= field.origin[k]
            || field.origin[k] + field.spacing <= field.origin[k]
        {
            return Err("invalid field sample bounds".into());
        }
    }
    for port in &field.ports {
        if u32::try_from(port.edge).is_err()
            || !port.along.is_finite()
            || !port.reference_flux.is_finite()
            || ![-1., 1.].contains(&port.outward_sign)
        {
            return Err("invalid field port values".into());
        }
    }
    // Verify total encoded size even on platforms with narrower address space.
    count
        .checked_mul(9)
        .and_then(|n| {
            field
                .ports
                .len()
                .checked_mul(16)
                .and_then(|p| n.checked_add(p))
        })
        .and_then(|n| n.checked_add(HEADER_BYTES))
        .ok_or("field encoded length overflow")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn field() -> PrecomputedField {
        PrecomputedField {
            geometry_revision: 0x0102030405060708,
            viscosity: 0.2,
            reference_pressure: 1.,
            max_scale: 2.,
            origin: [-2., 3.],
            spacing: 0.5,
            size: [2, 2],
            velocity: vec![[1., 2.], [3., 4.], [0., 0.], [7., 8.]],
            fluid: vec![true, true, false, true],
            ports: vec![FieldPort {
                edge: 7,
                along: 0.25,
                outward_sign: -1.,
                reference_flux: -3.,
            }],
        }
    }
    #[test]
    fn binary_round_trip_preserves_all_fields_and_row_order() {
        let source = field();
        let mut bytes = Vec::new();
        write_field(&mut bytes, &source).unwrap();
        assert_eq!(&bytes[..8], b"BFLOW001");
        assert_eq!(&bytes[8..16], &[8, 7, 6, 5, 4, 3, 2, 1]);
        assert_eq!(bytes.len(), 52 + 16 + 4 * 9);
        let decoded = read_field(bytes.as_slice()).unwrap();
        assert_eq!(decoded.geometry_revision, source.geometry_revision);
        assert_eq!(decoded.viscosity, source.viscosity);
        assert_eq!(decoded.reference_pressure, source.reference_pressure);
        assert_eq!(decoded.max_scale, source.max_scale);
        assert_eq!(decoded.origin, source.origin);
        assert_eq!(decoded.spacing, source.spacing);
        assert_eq!(decoded.size, source.size);
        assert_eq!(decoded.velocity, source.velocity);
        assert_eq!(decoded.fluid, source.fluid);
        let port = &decoded.ports[0];
        assert_eq!(
            (
                port.edge,
                port.along,
                port.outward_sign,
                port.reference_flux
            ),
            (7, 0.25, -1., -3.)
        );
    }
    #[test]
    fn malformed_assets_are_rejected() {
        let mut bytes = Vec::new();
        write_field(&mut bytes, &field()).unwrap();
        assert!(read_field(&bytes[..7]).is_err());
        assert!(read_field(&bytes[..bytes.len() - 1]).is_err());
        let mut bad = bytes.clone();
        bad[7] = b'2';
        assert!(read_field(bad.as_slice()).is_err());
        let mut bad = bytes.clone();
        bad.push(0);
        assert!(read_field(bad.as_slice()).is_err());
        let mut bad = bytes.clone();
        *bad.last_mut().unwrap() = 2;
        assert!(read_field(bad.as_slice()).is_err());
        let mut bad = bytes.clone();
        bad[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_field(bad.as_slice()).is_err());
        let mut invalid = field();
        invalid.velocity[0][0] = f32::NAN;
        assert!(write_field(Vec::new(), &invalid).is_err());
        let mut bad = bytes;
        bad[16..20].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(read_field(bad.as_slice()).is_err());
    }
}
