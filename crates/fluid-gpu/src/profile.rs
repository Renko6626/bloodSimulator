//! Opt-in wall timers and pass timestamps. Never required by a solver backend.
use crate::{LabResult, legacy::Context};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::mpsc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub struct Dispatch {
    pub label: &'static str,
    pub groups: u64,
}
#[derive(Clone, Default, Serialize)]
pub struct Stage {
    pub ms: f64,
    pub dispatches: u64,
    pub single_workgroup_dispatches: u64,
    pub workgroups: u64,
}
pub struct Queries {
    pub set: wgpu::QuerySet,
    pub resolve: wgpu::Buffer,
    pub readback: wgpu::Buffer,
    pub capacity: usize,
}
pub struct Profiler {
    pub queries: Option<Queries>,
    pub pending: Vec<Dispatch>,
    host: BTreeMap<&'static str, f64>,
    detail: BTreeMap<&'static str, f64>,
    stages: BTreeMap<&'static str, Stage>,
    period_ns: f64,
    span_ms: f64,
    collections: u64,
    bytes: u64,
}
impl Profiler {
    pub fn new(context: &Context, max_dispatches: usize) -> Self {
        let queries = if context
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
            && max_dispatches * 2 <= wgpu::QUERY_SET_MAX_QUERIES as usize
        {
            let bytes = max_dispatches as u64 * 16;
            Some(Queries {
                set: context.device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("Stokes profile"),
                    ty: wgpu::QueryType::Timestamp,
                    count: max_dispatches as u32 * 2,
                }),
                resolve: context.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Stokes timestamp resolve"),
                    size: bytes,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                readback: context.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Stokes timestamp readback"),
                    size: bytes,
                    usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                    mapped_at_creation: false,
                }),
                capacity: max_dispatches,
            })
        } else {
            None
        };
        Self {
            queries,
            pending: Vec::new(),
            host: BTreeMap::new(),
            detail: BTreeMap::new(),
            stages: BTreeMap::new(),
            period_ns: context.queue.get_timestamp_period() as f64,
            span_ms: 0.,
            collections: 0,
            bytes: 0,
        }
    }
    pub fn reset(&mut self) {
        assert!(self.pending.is_empty());
        self.host.clear();
        self.detail.clear();
        self.stages.clear();
        self.span_ms = 0.;
        self.collections = 0;
        self.bytes = 0;
    }
    pub fn mark(&mut self, label: &'static str, elapsed: Duration) {
        *self.host.entry(label).or_default() += elapsed.as_secs_f64() * 1000.;
    }
    pub fn detail(&mut self, label: &'static str, elapsed: Duration) {
        *self.detail.entry(label).or_default() += elapsed.as_secs_f64() * 1000.;
    }
    pub fn collect(&mut self, context: &Context) -> LabResult<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let start = Instant::now();
        let q = self.queries.as_ref().unwrap();
        let bytes = self.pending.len() as u64 * 16;
        let slice = q.readback.slice(0..bytes);
        let (tx, rx) = mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        context.device.poll(wgpu::PollType::wait_indefinitely())?;
        rx.recv()??;
        let mapped = slice.get_mapped_range();
        let ticks: &[u64] = bytemuck::cast_slice(&mapped);
        self.span_ms += aggregate(&mut self.stages, &self.pending, ticks, self.period_ns);
        drop(mapped);
        q.readback.unmap();
        self.pending.clear();
        self.collections += 1;
        self.bytes += bytes;
        self.mark("profile_collect_ms", start.elapsed());
        Ok(())
    }
    pub fn report(&self, elapsed: Duration) -> Value {
        let total_ms = elapsed.as_secs_f64() * 1000.;
        let accounted_ms: f64 = self.host.values().sum();
        json!({"host_ms":self.host,"host_detail_ms":self.detail,"total_ms":total_ms,
            "unaccounted_ms":total_ms-accounted_ms,
            "gpu_timestamps_enabled":self.queries.is_some(),
            "gpu_timestamp_status":if self.queries.is_some() {"enabled"} else {"not_requested_or_unsupported_or_batch_too_large"},
            "timestamp_period_ns":self.period_ns,"gpu_stages":self.stages,
            "gpu_interval_span_ms":self.span_ms,
            "profile_readbacks":self.collections,"profile_readback_bytes":self.bytes,
            "notes":"Opt-in diagnostics perturb timing. Pass durations exclude gaps; interval span includes gaps/idle between passes. Host timers and GPU timeline overlap. Planned gated dispatches are included."})
    }
}
fn aggregate(
    stages: &mut BTreeMap<&'static str, Stage>,
    labels: &[Dispatch],
    ticks: &[u64],
    period_ns: f64,
) -> f64 {
    assert_eq!(ticks.len(), labels.len() * 2);
    for (label, pair) in labels.iter().zip(ticks.chunks_exact(2)) {
        let stage = stages.entry(label.label).or_default();
        stage.ms += pair[1].wrapping_sub(pair[0]) as f64 * period_ns / 1e6;
        stage.dispatches += 1;
        stage.single_workgroup_dispatches += u64::from(label.groups == 1);
        stage.workgroups += label.groups;
    }
    ticks.last().unwrap().wrapping_sub(ticks[0]) as f64 * period_ns / 1e6
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamps_convert_period_and_keep_interpass_gaps_separate() {
        let labels = [
            Dispatch {
                label: "smooth",
                groups: 1,
            },
            Dispatch {
                label: "spmv",
                groups: 3,
            },
            Dispatch {
                label: "smooth",
                groups: 2,
            },
        ];
        let mut stages = BTreeMap::new();
        let span = aggregate(
            &mut stages,
            &labels,
            &[100, 110, 120, 135, 140, 150],
            2_000.,
        );
        assert!((span - 0.1).abs() < 1e-12);
        assert!((stages["smooth"].ms - 0.04).abs() < 1e-12);
        assert!((stages["spmv"].ms - 0.03).abs() < 1e-12);
        assert_eq!(stages["smooth"].dispatches, 2);
        assert_eq!(stages["smooth"].single_workgroup_dispatches, 1);
        assert_eq!(stages["smooth"].workgroups, 3);
    }
}
