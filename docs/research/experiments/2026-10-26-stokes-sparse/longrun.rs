//! Research probe: paired tolerance comparison with frame diagnostics and
//! one-second cell samples. Does not change simulation dynamics or input.
use rapier2d::prelude::Vector;
use simulation::{Config, MoveInput, Simulation};
use std::fmt::Write as _;
use std::io::{BufWriter, Write};
use std::path::Path;

fn area(points: &[[f32; 2]]) -> f64 {
    (0..points.len())
        .map(|i| {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            a[0] as f64 * b[1] as f64 - a[1] as f64 * b[0] as f64
        })
        .sum::<f64>()
        .abs()
        * 0.5
}

fn center(points: &[[f32; 2]]) -> Vector {
    points
        .iter()
        .map(|p| Vector::from_array(*p))
        .sum::<Vector>()
        / points.len() as f32
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let seconds: f64 = args[1].parse().unwrap();
    let out = Path::new(&args[2]);
    let config = Config::default();
    let dt = config.dt as f64;
    let count = config.cell_count;
    let steps = (seconds / dt).round() as usize;
    let mut sim = Simulation::new_local_world(config);
    let map = sim.world_map().unwrap().clone();
    let initial = sim.snapshot();
    let initial_areas: Vec<_> = initial.cells.iter().map(|p| area(p)).collect();
    let mut previous: Vec<_> = initial.cells.iter().map(|p| center(p)).collect();
    let mut previous_edges: Vec<_> = previous.iter().map(|p| map.nearest_edge(*p)).collect();
    let mut progress = vec![0.0_f64; count];
    let mut frames = BufWriter::new(std::fs::File::create(out.join("frames.csv")).unwrap());
    let mut cells = BufWriter::new(std::fs::File::create(out.join("cells.csv")).unwrap());
    let mut events = BufWriter::new(std::fs::File::create(out.join("crossings.csv")).unwrap());
    let mut arrows = BufWriter::new(std::fs::File::create(out.join("arrows.csv")).unwrap());
    writeln!(frames,"frame,time,whole_ms,fluid_ms,iterations,active,contacts,divergence_peak,momentum_peak,exchange,net_flux_last,in_volume,out_volume,abs_net_volume,upper_volume,lower_volume,flux_relative_peak,grid_speed_peak,force_peak,point_divergence_peak,max_area_error,max_wall_overshoot,max_center_speed,mean_center_speed,robot_x,robot_y").unwrap();
    writeln!(cells, "frame,time,cell,x,y,area,edge,distance").unwrap();
    writeln!(events, "frame,time,cell,from,to").unwrap();
    writeln!(arrows, "frame,time,arrow,x,y,vx,vy").unwrap();
    for (i, p) in previous.iter().enumerate() {
        writeln!(
            cells,
            "0,0,{i},{:.9e},{:.9e},{:.17e},{},0",
            p.x, p.y, initial_areas[i], previous_edges[i]
        )
        .unwrap();
    }
    for step in 1..=steps {
        sim.tick(MoveInput::default());
        let state = sim.snapshot();
        let f = state.fluid.as_ref().unwrap();
        let l = &f.long;
        let mut max_area_error = 0.0_f64;
        let mut max_wall_overshoot = 0.0_f32;
        let mut max_speed = 0.0_f64;
        let mut sum_speed = 0.0_f64;
        let sample = step % 60 == 0 || step == 1 || step == steps;
        for (i, points) in state.cells.iter().enumerate() {
            assert!(
                points.iter().all(|p| p.iter().all(|x| x.is_finite())),
                "nonfinite cell {i} at step {step}"
            );
            let p = center(points);
            let distance = (p - previous[i]).length() as f64;
            progress[i] += distance;
            max_speed = max_speed.max(distance / dt);
            sum_speed += distance / dt;
            let a = area(points);
            max_area_error = max_area_error.max((a / initial_areas[i] - 1.0).abs());
            for point in points {
                max_wall_overshoot =
                    max_wall_overshoot.max(map.signed_distance(Vector::from_array(*point)));
            }
            let edge = map.nearest_edge(p);
            if edge != previous_edges[i] && map.edges[previous_edges[i]].to == map.edges[edge].from
            {
                writeln!(
                    events,
                    "{step},{:.9},{i},{},{edge}",
                    step as f64 * dt,
                    previous_edges[i]
                )
                .unwrap();
            }
            if sample {
                writeln!(
                    cells,
                    "{step},{:.9},{i},{:.9e},{:.9e},{a:.17e},{edge},{:.17e}",
                    step as f64 * dt,
                    p.x,
                    p.y,
                    progress[i]
                )
                .unwrap();
            }
            previous[i] = p;
            previous_edges[i] = edge;
        }
        writeln!(frames,"{step},{:.9},{:.12},{:.12},{},{},{},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.17e},{:.9e},{:.9e}",step as f64*dt,state.solve_ms,f.solve_ms,f.pressure_iterations,f.active_cells,state.soft_contacts,l.divergence_peak,l.momentum_peak,f.exchange_error,f.net_flux,l.in_volume,l.out_volume,l.abs_net_volume,l.upper_volume,l.lower_volume,l.flux_relative_peak,l.grid_speed_peak,l.force_peak,l.point_divergence_peak,max_area_error,max_wall_overshoot,max_speed,sum_speed/count as f64,state.robot[0],state.robot[1]).unwrap();
        if sample {
            for (i, a) in state.arrows.iter().enumerate() {
                writeln!(
                    arrows,
                    "{step},{:.9},{i},{:.9e},{:.9e},{:.9e},{:.9e}",
                    step as f64 * dt,
                    a.position[0],
                    a.position[1],
                    a.velocity[0],
                    a.velocity[1]
                )
                .unwrap();
            }
            frames.flush().unwrap();
            cells.flush().unwrap();
            events.flush().unwrap();
            arrows.flush().unwrap();
        }
        if step % 600 == 0 || step == steps {
            let mut message = String::new();
            write!(message,"progress frame={step}/{steps} time={:.1} active={} grid_speed={:.4} area_error={:.4}%",step as f64*dt,f.active_cells,l.grid_speed_peak,max_area_error*100.0).unwrap();
            println!("{message}");
        }
    }
}
