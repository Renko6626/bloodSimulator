//! Injected only into the frozen experiment copy of fluid::linear.
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_REGION: AtomicUsize = AtomicUsize::new(0);

pub(super) struct Capture {
    root: PathBuf,
    calls: AtomicUsize,
}

fn binary(path: PathBuf, values: &[f64]) -> std::io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    for v in values {
        out.write_all(&v.to_le_bytes())?;
    }
    out.flush()
}

impl Capture {
    pub(super) fn new(
        nv: usize,
        np: usize,
        entries: &[faer::sparse::Triplet<usize, usize, f64>],
    ) -> Result<Option<Self>, String> {
        let Some(root) = std::env::var_os("BLOOD_GPU_CAPTURE") else {
            return Ok(None);
        };
        let id = NEXT_REGION.fetch_add(1, Ordering::Relaxed);
        let root = PathBuf::from(root).join(format!("region-{id}"));
        let save = || -> std::io::Result<()> {
            std::fs::create_dir(&root)?;
            let mut rows = vec![BTreeMap::<usize, f64>::new(); nv + np];
            for t in entries {
                *rows[t.row].entry(t.col).or_default() += t.val;
            }
            for row in &mut rows {
                row.retain(|_, a| *a != 0.0);
            }
            let nnz: usize = rows.iter().map(|r| r.len()).sum();
            let mut mm = BufWriter::new(File::create(root.join("matrix.mtx"))?);
            writeln!(mm, "%%MatrixMarket matrix coordinate real general")?;
            writeln!(mm, "{} {} {nnz}", nv + np, nv + np)?;
            let mut ptr = BufWriter::new(File::create(root.join("row_ptr.u32"))?);
            let mut cols = BufWriter::new(File::create(root.join("columns.u32"))?);
            let mut vals = BufWriter::new(File::create(root.join("values.f64"))?);
            let mut offset = 0_u32;
            ptr.write_all(&offset.to_le_bytes())?;
            for (i, row) in rows.iter().enumerate() {
                for (&j, &a) in row {
                    writeln!(mm, "{} {} {a:.17e}", i + 1, j + 1)?;
                    cols.write_all(&(j as u32).to_le_bytes())?;
                    vals.write_all(&a.to_le_bytes())?;
                    offset += 1;
                }
                ptr.write_all(&offset.to_le_bytes())?;
            }
            mm.flush()?;
            ptr.flush()?;
            cols.flush()?;
            vals.flush()?;
            std::fs::write(
                root.join("matrix.json"),
                format!(
                    "{{\"format_version\":1,\"rows\":{},\"velocity_count\":{nv},\"pressure_count\":{np},\"nnz\":{nnz}}}\n",
                    nv + np
                ),
            )?;
            std::fs::write(root.join("samples.csv"), "sample,capture_cpu_solve_ms\n")?;
            Ok(())
        };
        save().map_err(|e| format!("capture matrix: {e}"))?;
        Ok(Some(Self {
            root,
            calls: AtomicUsize::new(0),
        }))
    }

    pub(super) fn domain(
        &self,
        n: usize,
        h: f64,
        origin: [f64; 2],
        cells: &[usize],
        u: &[[f64; 2]],
        v: &[[f64; 2]],
    ) {
        std::fs::write(
            self.root.join("domain.json"),
            format!("{{\"n\":{n},\"h\":{h},\"origin\":{origin:?},\"pressure_cells\":{cells:?},\"u_positions\":{u:?},\"v_positions\":{v:?}}}\n"),
        ).expect("write captured domain");
    }

    pub(super) fn record(&self, rhs: &[f64], solution: &[f64], solve_ms: f64) {
        let call = self.calls.fetch_add(1, Ordering::Relaxed);
        if ![0, 1, 3, 7, 15, 31, 63, 127].contains(&call) {
            return;
        }
        binary(self.root.join(format!("rhs-{call:03}.f64")), rhs).expect("write captured RHS");
        binary(self.root.join(format!("solution-{call:03}.f64")), solution)
            .expect("write captured reference");
        let mut log = OpenOptions::new()
            .append(true)
            .open(self.root.join("samples.csv"))
            .unwrap();
        writeln!(log, "{call},{solve_ms:.9}").unwrap();
    }
}
