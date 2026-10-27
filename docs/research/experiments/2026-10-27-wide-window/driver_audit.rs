//! One-variable static control: keep 16x16 extent, use the candidate's 0.5 spacing.
use simulation::{Config, Simulation, circulation::graph::GraphNetwork, map::MapDefinition};

fn main() {
    let map = MapDefinition::small_world();
    let flow = GraphNetwork::new(&map, 0.05).solve(1.0);
    let pump = &map.edges[map.pump_edge];
    let difference = flow.pressures[pump.from] - flow.pressures[pump.to];
    println!(
        "pump_edge={} from={} to={} pressure_from={} pressure_to={} pressure_difference={} head=1 net_drive={} rate={}",
        map.pump_edge,
        pump.from,
        pump.to,
        flow.pressures[pump.from],
        flow.pressures[pump.to],
        difference,
        difference + 1.0,
        flow.rates[map.pump_edge]
    );
    for (name, lo, hi) in [
        ("base", [-22.0, -8.0], [-6.0, 8.0]),
        ("wide", [-38.0, -24.0], [10.0, 24.0]),
    ] {
        let inside = pump
            .centerline
            .iter()
            .filter(|p| p[0] >= lo[0] && p[0] <= hi[0] && p[1] >= lo[1] && p[1] <= hi[1])
            .count();
        println!(
            "{name} pump_centerline_points_inside={inside}/{}",
            pump.centerline.len()
        );
    }
    let sim = Simulation::new_local_world(Config::default());
    let bounds = sim.snapshot().fluid.unwrap().bounds;
    println!("control_bounds={bounds:?}");
    for p in [[-16.0, 0.0], [-12.0, 0.0], [-10.0, -4.0]] {
        let (weight, velocity) = sim.window_trial_sample(p);
        println!("point={p:?} weight={weight} velocity={velocity:?}");
        assert!(weight >= 0.99999 && velocity[0] > 0.0);
    }
}
