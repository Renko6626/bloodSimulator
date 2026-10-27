//! Version-one portable CSR cases, compatible with the original GPU experiment.
use crate::{CsrMatrix, FluidResult, Geometry, StokesProblem};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Serialize, Deserialize)]
struct Metadata {
    format_version: u32,
    rows: usize,
    velocity_count: usize,
    pressure_count: usize,
    nnz: usize,
    #[serde(default = "unit_viscosity")]
    viscosity: f64,
}
fn unit_viscosity() -> f64 {
    1.
}
pub struct DatasetCase {
    pub problem: Arc<StokesProblem>,
    pub folder: PathBuf,
}
pub fn read_f64(path: &Path) -> FluidResult<Vec<f64>> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    if data.len() % 8 != 0 {
        return Err("invalid f64 file length".into());
    }
    let values: Vec<_> = data
        .chunks_exact(8)
        .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
        .collect();
    if values.iter().any(|v| !v.is_finite()) {
        return Err("non-finite dataset vector".into());
    }
    Ok(values)
}
fn read_u32(path: &Path) -> FluidResult<Vec<u32>> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    if data.len() % 4 != 0 {
        return Err("invalid u32 file length".into());
    }
    Ok(data
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect())
}
pub fn write_f64(path: &Path, values: &[f64]) -> FluidResult<()> {
    let mut file = BufWriter::new(std::fs::File::create(path).map_err(|e| e.to_string())?);
    for v in values {
        file.write_all(&v.to_le_bytes())
            .map_err(|e| e.to_string())?;
    }
    file.flush().map_err(|e| e.to_string())
}
pub fn write_problem(folder: &Path, p: &StokesProblem) -> FluidResult<()> {
    p.validate()?;
    std::fs::create_dir(folder).map_err(|e| e.to_string())?;
    let metadata = Metadata {
        format_version: 1,
        rows: p.matrix.rows,
        velocity_count: p.velocity_count,
        pressure_count: p.matrix.rows - p.velocity_count,
        nnz: p.matrix.values.len(),
        viscosity: p.viscosity,
    };
    std::fs::write(
        folder.join("matrix.json"),
        serde_json::to_vec_pretty(&metadata).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    for (name, values) in [
        ("row_ptr.u32", &p.matrix.ptr),
        ("columns.u32", &p.matrix.columns),
    ] {
        let mut file =
            BufWriter::new(std::fs::File::create(folder.join(name)).map_err(|e| e.to_string())?);
        for v in values {
            file.write_all(&v.to_le_bytes())
                .map_err(|e| e.to_string())?;
        }
        file.flush().map_err(|e| e.to_string())?;
    }
    write_f64(&folder.join("values.f64"), &p.matrix.values)?;
    let mut mm = BufWriter::new(
        std::fs::File::create(folder.join("matrix.mtx")).map_err(|e| e.to_string())?,
    );
    writeln!(
        mm,
        "%%MatrixMarket matrix coordinate real general\n{} {} {}",
        p.matrix.rows,
        p.matrix.rows,
        p.matrix.values.len()
    )
    .map_err(|e| e.to_string())?;
    for i in 0..p.matrix.rows {
        for (j, a) in p.matrix.row(i) {
            writeln!(mm, "{} {} {a:.17e}", i + 1, j + 1).map_err(|e| e.to_string())?;
        }
    }
    mm.flush().map_err(|e| e.to_string())?;
    if let Some(g) = &p.geometry {
        std::fs::write(folder.join("domain.json"), serde_json::to_vec(g).unwrap())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}
impl DatasetCase {
    pub fn load(folder: &Path) -> FluidResult<Self> {
        let m: Metadata = serde_json::from_slice(
            &std::fs::read(folder.join("matrix.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if m.format_version != 1 || m.rows != m.velocity_count + m.pressure_count {
            return Err("unsupported dataset schema".into());
        }
        let matrix = CsrMatrix {
            rows: m.rows,
            cols: m.rows,
            ptr: read_u32(&folder.join("row_ptr.u32"))?,
            columns: read_u32(&folder.join("columns.u32"))?,
            values: read_f64(&folder.join("values.f64"))?,
        };
        if matrix.values.len() != m.nnz {
            return Err("dataset nonzero count mismatch".into());
        }
        let geometry = if folder.join("domain.json").exists() {
            Some(
                serde_json::from_slice::<Geometry>(
                    &std::fs::read(folder.join("domain.json")).map_err(|e| e.to_string())?,
                )
                .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        let p = StokesProblem {
            matrix,
            velocity_count: m.velocity_count,
            viscosity: m.viscosity,
            geometry,
        };
        p.validate()?;
        Ok(Self {
            problem: Arc::new(p),
            folder: folder.into(),
        })
    }
    pub fn samples(&self) -> FluidResult<Vec<String>> {
        let mut ids = Vec::new();
        for entry in std::fs::read_dir(&self.folder).map_err(|e| e.to_string())? {
            let name = entry
                .map_err(|e| e.to_string())?
                .file_name()
                .to_string_lossy()
                .into_owned();
            if let Some(id) = name
                .strip_prefix("rhs-")
                .and_then(|s| s.strip_suffix(".f64"))
            {
                ids.push(id.to_string());
            }
        }
        ids.sort();
        if ids.is_empty() {
            return Err("dataset has no RHS samples".into());
        }
        Ok(ids)
    }
    pub fn sample(&self, id: &str) -> FluidResult<(Vec<f64>, Vec<f64>)> {
        let b = read_f64(&self.folder.join(format!("rhs-{id}.f64")))?;
        let x = read_f64(&self.folder.join(format!("solution-{id}.f64")))?;
        if b.len() != self.problem.matrix.rows || x.len() != b.len() {
            return Err("dataset vector dimension mismatch".into());
        }
        Ok((b, x))
    }
}
