//! Diagnostic only: freeze recorded cell geometry, set every marker velocity to
//! zero, and measure how a pre-existing flow decays or grows under lagged updates.
use std::error::Error;
use std::fs;
use stokes_hotpath_solver::fluid::{Marker, Stokes};

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args().nth(1).ok_or("expected outlines.csv")?;
    let text = fs::read_to_string(path)?;
    let rows: Vec<Vec<&str>> = text
        .lines()
        .skip(1)
        .map(|line| line.split(',').collect())
        .collect();
    let first: Vec<_> = rows.iter().filter(|row| row[3] == "1").collect();
    let count: usize = first.first().ok_or("no frame 1 outlines")?[2].parse()?;
    let mut markers = Vec::with_capacity(count * 4);
    for cell in 0..count {
        let mut nodes = [[0.0; 2]; 12];
        let mut seen = [false; 12];
        for row in first
            .iter()
            .filter(|row| row[5].parse::<usize>().ok() == Some(cell))
        {
            let index: usize = row[7].parse()?;
            nodes[index] = [row[8].parse()?, row[9].parse()?];
            seen[index] = true;
        }
        assert!(seen.iter().all(|x| *x));
        let center = [
            nodes.iter().map(|p| p[0]).sum::<f64>() / 12.0,
            nodes.iter().map(|p| p[1]).sum::<f64>() / 12.0,
        ];
        for index in [0, 3, 6, 9] {
            markers.push(Marker {
                position: [
                    (nodes[index][0] + center[0]) * 0.5,
                    (nodes[index][1] + center[1]) * 0.5,
                ],
                velocity: [0.0, 0.0],
                drag: 0.25 / 4.0,
            });
        }
    }
    let mut solver = Stokes::with_threads(64, 4)?;
    // Seed with a valid Poiseuille flow. Remove its drive before the first update.
    let mut prior = solver.solve(&[], [1.0, 0.0])?;
    let equilibrium = solver.solve(&markers, [0.0, 0.0])?;
    assert!(equilibrium.grid_velocity.iter().all(|u| *u == 0.0));
    let norm = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
    println!("cells,iteration,velocity_norm,norm_ratio,mean_x_velocity,force_law_rms");
    println!(
        "{count},0,{:.12e},0,{:.12e},0",
        norm(&prior.grid_velocity),
        prior.grid_velocity[..4096].iter().sum::<f64>() / 4096.0
    );
    for iteration in 1..=16 {
        let next = solver.solve_lagged(&markers, [0.0, 0.0], Some(&prior))?;
        println!(
            "{count},{iteration},{:.12e},{:.12e},{:.12e},{:.12e}",
            norm(&next.grid_velocity),
            norm(&next.grid_velocity) / norm(&prior.grid_velocity),
            next.grid_velocity[..4096].iter().sum::<f64>() / 4096.0,
            next.force_law_rms
        );
        prior = next;
    }
    Ok(())
}
