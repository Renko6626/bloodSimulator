#!/usr/bin/env python3
"""Only modifies an extracted d0dfa07 checkout, never the production tree."""
import argparse
from pathlib import Path
p = argparse.ArgumentParser()
p.add_argument('root', type=Path)
p.add_argument('--budget', type=float, required=True, choices=[0.05, 0.07])
a = p.parse_args()

def edit(path, before, after):
    file = a.root / path
    source = file.read_text()
    assert source.count(before) == 1, (path, before[:70], source.count(before))
    file.write_text(source.replace(before, after, 1))

edit('crates/simulation/src/fluid/lbm_coupling.rs',
     'let limit = (0.05 * h / options.velocity_bound)',
     f'let limit = ({a.budget} * h / options.velocity_bound)')
# Read the existing CPU field cache through its existing interpolation API.
edit('crates/simulation/src/runtime.rs', '    pub fn world_map(&self)', '''    #[cfg(feature = "lbm-gpu")]
    pub fn diagnostic_lbm_sample(&self, position: [f32; 2]) -> Result<blood_fluid_gpu::lbm::LocalSample, String> {
        self.lbm.as_ref().ok_or("LBM unavailable")?.window.sample(position)
    }

    pub fn world_map(&self)''')
edit('crates/simulation/examples/lbm_gameplay_probe.rs',
     '        let stats = sim.lbm_stats().unwrap();', '''        let stats = sim.lbm_stats().unwrap();
        writeln!(status, "DIAGNOSTIC lattice_dt={} steps_per_substep={} mechanical_dt={} viscosity={} velocity_bound={}", stats.lattice_dt, stats.lattice_steps_per_substep, config.dt / config.substeps as f32, config.viscosity, options.velocity_bound).map_err(|e|e.to_string())?;
        let mut fields = BufWriter::new(File::create(format!("{output}/{name}.field.csv")).map_err(|e|e.to_string())?);
        let mut nodes = BufWriter::new(File::create(format!("{output}/{name}.nodes.csv")).map_err(|e|e.to_string())?);
        writeln!(fields, "frame,time,x,y,rho,ux,uy").map_err(|e|e.to_string())?;
        writeln!(nodes, "frame,time,cell,node,x,y").map_err(|e|e.to_string())?;
        let mut previous_fluid_time = 0_f64;''')
edit('crates/simulation/examples/lbm_gameplay_probe.rs',
     '                if frame % 120 == 0 {', '''                let fluid_dt = l.elapsed_fluid_time - previous_fluid_time;
                if (fluid_dt - config.dt as f64).abs() > 1e-8 {
                    return Err(format!("fluid/mechanical time mismatch at {frame}: {fluid_dt}"));
                }
                previous_fluid_time = l.elapsed_fluid_time;
                if w.lattice_steps != (config.substeps * l.lattice_steps_per_substep) as u64 {
                    return Err(format!("unexpected lattice work at {frame}: {}", w.lattice_steps));
                }
                if frame % 120 == 119 || frame + 1 == warmup + frames {
                    let h = options.profile.cell_spacing();
                    let n = options.profile.cells_per_side();
                    for y in 0..n {
                        for x in 0..n {
                            let p = [l.bounds[0][0] + (x as f32 + 0.5) * h,
                                     l.bounds[0][1] + (y as f32 + 0.5) * h];
                            let sample = sim.diagnostic_lbm_sample(p)?;
                            if sample.status == blood_fluid_gpu::lbm::LocalSampleStatus::Valid {
                                writeln!(fields, "{frame},{},{},{},{},{},{}", snapshot.time, p[0], p[1], sample.density, sample.velocity[0], sample.velocity[1]).map_err(|e|e.to_string())?;
                            }
                        }
                    }
                    for (cell, contour) in snapshot.cells.iter().enumerate() {
                        for (node, p) in contour.iter().enumerate() {
                            writeln!(nodes, "{frame},{},{cell},{node},{},{}", snapshot.time, p[0], p[1]).map_err(|e|e.to_string())?;
                        }
                    }
                    fields.flush().map_err(|e|e.to_string())?;
                    nodes.flush().map_err(|e|e.to_string())?;
                }
                if frame % 120 == 0 {''')
