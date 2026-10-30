//! Aggregate ordinary-cell traffic outside the detailed mechanical window.
//! Stocks include blocked/fractional near-field quantities; there is no second
//! boundary queue. This is a mixing model, not individual travel-time tracking.
use crate::{circulation::graph::GraphFlow, map::MapDefinition};
use rapier2d::prelude::Vector;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellPopulationOptions {
    pub activation_half_extent: f32,
    pub retention_half_extent: f32,
    pub protected_half_extent: f32,
    pub transport_interval: f64,
}
impl Default for CellPopulationOptions {
    fn default() -> Self {
        Self {
            activation_half_extent: 36.,
            retention_half_extent: 40.,
            protected_half_extent: 32.,
            transport_interval: 0.2,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum CellSimulationMode {
    #[default]
    Full,
    Local(CellPopulationOptions),
}

struct Interval {
    start: f64,
    end: f64,
    near: bool,
    count: f64,
}

pub(crate) struct CellPopulation {
    edges: Vec<Vec<Interval>>,
    pub options: CellPopulationOptions,
    center: Vector,
    elapsed: f64,
    pub total_count: usize,
    pub spawned: usize,
    pub retired: usize,
    pub exchange_ms: f64,
}
impl CellPopulation {
    pub fn new(map: &MapDefinition, center: Vector, options: CellPopulationOptions) -> Self {
        assert!(
            options.protected_half_extent.is_finite()
                && options.protected_half_extent >= 0.
                && options.activation_half_extent > options.protected_half_extent
                && options.retention_half_extent.is_finite()
                && options.retention_half_extent >= options.activation_half_extent
                && options.transport_interval.is_finite()
                && options.transport_interval > 0.
        );
        Self {
            edges: partition(map, center, options.activation_half_extent),
            options,
            center,
            elapsed: 0.,
            total_count: 0,
            spawned: 0,
            retired: 0,
            exchange_ms: 0.,
        }
    }
    pub fn contains(&self, position: Vector) -> bool {
        (position - self.center).abs().max_element() <= self.options.activation_half_extent
    }
    pub fn abstract_count(&self) -> f64 {
        self.edges.iter().flatten().map(|i| i.count).sum()
    }
    pub fn deposit(&mut self, map: &MapDefinition, position: Vector, amount: f64) {
        let edge = map.nearest_edge(position);
        let along = map.project_edge(edge, position).along as f64;
        let intervals = &mut self.edges[edge];
        let index = intervals
            .iter()
            .position(|i| along < i.end)
            .unwrap_or(intervals.len() - 1);
        intervals[index].count += amount;
    }
    pub fn repartition(&mut self, map: &MapDefinition, center: Vector) {
        if center == self.center {
            return;
        }
        let mut next = partition(map, center, self.options.activation_half_extent);
        for (old, new) in self.edges.iter().zip(&mut next) {
            for a in old {
                if a.count == 0. {
                    continue;
                }
                let overlaps: Vec<_> = new
                    .iter()
                    .enumerate()
                    .filter_map(|(index, b)| {
                        let length = a.end.min(b.end) - a.start.max(b.start);
                        (length > 0.).then_some((index, length))
                    })
                    .collect();
                let length: f64 = overlaps.iter().map(|(_, l)| l).sum();
                let mut remaining = a.count;
                for (n, &(index, overlap)) in overlaps.iter().enumerate() {
                    let amount = if n + 1 == overlaps.len() {
                        remaining
                    } else {
                        (a.count * overlap / length).min(remaining)
                    };
                    new[index].count += amount;
                    remaining -= amount;
                }
            }
        }
        self.edges = next;
        self.center = center;
    }
    /// All transfers read the old stocks, so incoming stock waits until the
    /// next coarse tick. Near intervals never transport abstract quantities.
    fn transport(&mut self, map: &MapDefinition, flow: &GraphFlow, dt: f64) {
        let mut delta: Vec<Vec<f64>> = self.edges.iter().map(|e| vec![0.; e.len()]).collect();
        for (edge, intervals) in self.edges.iter().enumerate() {
            let rate = flow.rates[edge] as f64;
            if rate == 0. {
                continue;
            }
            let velocity =
                rate / (std::f64::consts::PI * (map.edges[edge].half_width as f64).powi(2));
            for (index, interval) in intervals.iter().enumerate() {
                if interval.near || interval.count == 0. {
                    continue;
                }
                let amount = interval.count
                    * (velocity.abs() * dt / (interval.end - interval.start)).min(1.);
                let adjacent = if rate > 0. {
                    (index + 1 < intervals.len()).then_some(index + 1)
                } else {
                    index.checked_sub(1)
                };
                if let Some(next) = adjacent {
                    delta[edge][index] -= amount;
                    delta[edge][next] += amount;
                    continue;
                }
                let node = if rate > 0. {
                    map.edges[edge].to
                } else {
                    map.edges[edge].from
                };
                let outgoing: Vec<_> = map
                    .edges
                    .iter()
                    .enumerate()
                    .filter_map(|(e, definition)| {
                        let q = flow.rates[e] as f64;
                        if q > 0. && definition.from == node {
                            Some((e, 0, q))
                        } else if q < 0. && definition.to == node {
                            Some((e, self.edges[e].len() - 1, -q))
                        } else {
                            None
                        }
                    })
                    .collect();
                let total_rate: f64 = outgoing.iter().map(|(_, _, q)| q).sum();
                if total_rate == 0. {
                    continue;
                } // Closed/dead end: retain stock.
                delta[edge][index] -= amount;
                let mut remaining = amount;
                for (n, &(e, i, q)) in outgoing.iter().enumerate() {
                    let share = if n + 1 == outgoing.len() {
                        remaining
                    } else {
                        (amount * q / total_rate).min(remaining)
                    };
                    delta[e][i] += share;
                    remaining -= share;
                }
            }
        }
        for (edge, changes) in self.edges.iter_mut().zip(delta) {
            for (interval, change) in edge.iter_mut().zip(changes) {
                interval.count += change;
            }
        }
    }
    pub fn advance(&mut self, map: &MapDefinition, flow: &GraphFlow, dt: f64) -> bool {
        self.elapsed += dt;
        let mut updated = false;
        while self.elapsed >= self.options.transport_interval {
            self.transport(map, flow, self.options.transport_interval);
            self.elapsed -= self.options.transport_interval;
            updated = true;
        }
        updated
    }
    /// Candidate positions carry their authoritative interval. The caller
    /// checks live contacts before consuming one unit with `materialize`.
    pub fn candidates(
        &self,
        map: &MapDefinition,
        flow: &GraphFlow,
        radius: f32,
    ) -> Vec<(usize, usize, Vector)> {
        let mut candidates = Vec::new();
        for (edge, intervals) in self.edges.iter().enumerate() {
            let offset = (map.edges[edge].half_width - radius - 0.16).max(0.) * 0.65;
            for (index, interval) in intervals.iter().enumerate() {
                if !interval.near || interval.count < 1. {
                    continue;
                }
                let samples = ((interval.end - interval.start) / (2. * radius + 0.3) as f64)
                    .ceil()
                    .max(1.) as usize;
                for n in 0..samples {
                    let n = if flow.rates[edge] < 0. {
                        samples - 1 - n
                    } else {
                        n
                    };
                    let along = interval.start
                        + (interval.end - interval.start) * (n as f64 + 0.5) / samples as f64;
                    for lane in [0., -offset, offset] {
                        let p = map.edge_point(edge, along as f32, lane);
                        let distance = (p - self.center).abs().max_element();
                        if distance > self.options.protected_half_extent
                            && distance <= self.options.activation_half_extent
                        {
                            candidates.push((edge, index, p));
                        }
                    }
                }
            }
        }
        candidates
    }
    pub fn materialize(&mut self, edge: usize, interval: usize) -> bool {
        let count = &mut self.edges[edge][interval].count;
        if *count < 1. {
            return false;
        }
        *count -= 1.;
        self.spawned += 1;
        true
    }
}

/// Clip each polyline segment, retaining arc order even if an edge leaves and
/// re-enters the square. Adjacent equal classifications form one mixing stock.
fn partition(map: &MapDefinition, center: Vector, half: f32) -> Vec<Vec<Interval>> {
    map.edges
        .iter()
        .map(|edge| {
            let mut intervals: Vec<Interval> = Vec::new();
            let mut arc = 0.;
            for segment in edge.centerline.windows(2) {
                let a = Vector::from_array(segment[0]);
                let d = Vector::from_array(segment[1]) - a;
                let length = d.length() as f64;
                if length == 0. {
                    continue;
                }
                let mut cuts = vec![0., 1.];
                let mut enter: f64 = 0.;
                let mut exit: f64 = 1.;
                for (start, direction, c) in [(a.x, d.x, center.x), (a.y, d.y, center.y)] {
                    if direction == 0. {
                        if (start - c).abs() > half {
                            enter = 1.;
                            exit = 0.;
                        }
                    } else {
                        let t0 = (c as f64 - half as f64 - start as f64) / direction as f64;
                        let t1 = (c as f64 + half as f64 - start as f64) / direction as f64;
                        enter = enter.max(t0.min(t1));
                        exit = exit.min(t0.max(t1));
                    }
                }
                if enter < exit {
                    cuts.extend([enter.clamp(0., 1.), exit.clamp(0., 1.)]);
                }
                cuts.sort_by(f64::total_cmp);
                cuts.dedup();
                for pair in cuts.windows(2) {
                    let t = (pair[0] + pair[1]) * 0.5;
                    let near =
                        [(a.x, d.x, center.x), (a.y, d.y, center.y)]
                            .iter()
                            .all(|&(s, d, c)| {
                                (s as f64 + t * d as f64 - c as f64).abs() <= half as f64
                            });
                    let start = arc + pair[0] * length;
                    let end = arc + pair[1] * length;
                    if let Some(previous) = intervals.last_mut().filter(|p| p.near == near) {
                        previous.end = end;
                    } else {
                        intervals.push(Interval {
                            start,
                            end,
                            near,
                            count: 0.,
                        });
                    }
                }
                arc += length;
            }
            intervals
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circulation::{Flow, graph::GraphFlow};
    use crate::map::{MapDefinition, VesselEdge};
    use rapier2d::prelude::Vector;
    use std::f32::consts::PI;

    fn map(nodes: &[[f32; 2]], pairs: &[(usize, usize)]) -> MapDefinition {
        MapDefinition {
            fluid: None,
            nodes: nodes.to_vec(),
            edges: pairs
                .iter()
                .map(|&(from, to)| VesselEdge {
                    from,
                    to,
                    half_width: 1.,
                    centerline: vec![nodes[from], nodes[to]],
                })
                .collect(),
            pump_edge: 0,
            spawn: nodes[0],
            landmarks: vec![],
        }
    }
    fn flow(rates: Vec<f32>) -> GraphFlow {
        GraphFlow {
            pressures: vec![],
            rates,
            summary: Flow::default(),
        }
    }
    fn remote(map: &MapDefinition) -> CellPopulation {
        CellPopulation::new(map, Vector::splat(1000.), CellPopulationOptions::default())
    }
    fn edge_count(pop: &CellPopulation, edge: usize) -> f64 {
        pop.edges[edge].iter().map(|i| i.count).sum()
    }
    #[test]
    fn births_prioritize_the_inflow_side_for_either_flow_direction() {
        let map = map(&[[-100., 0.], [100., 0.]], &[(0, 1)]);
        let mut pop = CellPopulation::new(&map, Vector::ZERO, Default::default());
        pop.deposit(&map, Vector::ZERO, 3.);
        let incoming = pop.candidates(&map, &flow(vec![PI]), 0.7);
        assert!(incoming[0].2.x < -32.);
        let reversed = pop.candidates(&map, &flow(vec![-PI]), 0.7);
        assert!(reversed[0].2.x > 32.);
    }
    #[test]
    fn stopped_flow_preserves_stock() {
        let map = map(&[[0., 0.], [10., 0.]], &[(0, 1)]);
        let mut pop = remote(&map);
        pop.deposit(&map, Vector::new(5., 0.), 7.25);
        pop.transport(&map, &flow(vec![0.]), 0.2);
        assert_eq!(pop.abstract_count(), 7.25);
        assert_eq!(edge_count(&pop, 0), 7.25);
    }
    #[test]
    fn transfer_uses_old_stock_and_only_one_edge_per_update() {
        let map = map(
            &[[0., 0.], [10., 0.], [10., 10.]],
            &[(0, 1), (1, 2), (2, 0)],
        );
        let mut pop = remote(&map);
        pop.deposit(&map, Vector::new(5., 0.), 10.);
        pop.transport(&map, &flow(vec![PI; 3]), 0.2);
        assert!((edge_count(&pop, 0) - 9.8).abs() < 1e-7);
        assert!((edge_count(&pop, 1) - 0.2).abs() < 1e-7);
        assert_eq!(edge_count(&pop, 2), 0.);
        assert!((pop.abstract_count() - 10.).abs() < 1e-12);
    }
    #[test]
    fn junction_distributes_outgoing_quantity_by_flow() {
        let map = map(
            &[[0., 0.], [10., 0.], [20., 10.], [20., -10.]],
            &[(0, 1), (1, 2), (1, 3)],
        );
        let mut pop = remote(&map);
        pop.deposit(&map, Vector::new(5., 0.), 10.);
        pop.transport(&map, &flow(vec![PI, 0.7 * PI, 0.3 * PI]), 0.2);
        assert!((edge_count(&pop, 1) - 0.14).abs() < 1e-7);
        assert!((edge_count(&pop, 2) - 0.06).abs() < 1e-7);
        assert!((pop.abstract_count() - 10.).abs() < 1e-12);
    }
    #[test]
    fn separated_far_intervals_do_not_bypass_near_region() {
        let mut map = map(&[[-100., 0.], [-100., 100.]], &[(0, 1)]);
        map.edges[0].centerline = vec![[-100., 0.], [100., 0.], [100., 100.], [-100., 100.]];
        let mut pop = CellPopulation::new(&map, Vector::ZERO, CellPopulationOptions::default());
        pop.deposit(&map, Vector::new(-80., 0.), 10.);
        assert_eq!(pop.edges[0].len(), 3);
        assert!(!pop.edges[0][0].near && pop.edges[0][1].near && !pop.edges[0][2].near);
        pop.transport(&map, &flow(vec![PI]), 0.2);
        assert!(pop.edges[0][1].count > 0.);
        assert_eq!(pop.edges[0][2].count, 0.);
        for n in 0..180 {
            pop.repartition(&map, Vector::new(n as f32 * 0.2, 0.));
            assert!((pop.abstract_count() - 10.).abs() < 1e-10);
            assert!(pop.edges.iter().flatten().all(|i| i.count >= 0.));
        }
    }
}
