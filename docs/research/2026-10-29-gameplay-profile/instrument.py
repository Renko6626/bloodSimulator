#!/usr/bin/env python3
"""Apply diagnostic-only edits to a clean git archive, never the working tree."""
from pathlib import Path
import sys
root = Path(sys.argv[1]).resolve()
if (root / '.git').exists():
    raise SystemExit('Use an extracted git archive, not a checkout')

def edit(name, replacements):
    p = root / name
    s = p.read_text()
    for old, new, count in replacements:
        assert s.count(old) == count, (name, old[:80], s.count(old), count)
        s = s.replace(old, new)
    p.write_text(s)

edit('crates/simulation/src/runtime.rs', [
    ('        let time = self.steps as f32 * self.config.dt;', '        let profile_inner = Instant::now();\n        let time = self.steps as f32 * self.config.dt;', 1),
    ('            robot::drive(', '            let profile_advection = Instant::now();\n            robot::drive(', 1),
    ('            if let Some(fluid) = &mut self.local_fluid {\n                fluid\n                    .advance(', '''            if std::env::var_os("BLOOD_PROFILE").is_some() {
                eprintln!("PROFILE_ADVECTION frame={} ms={:.6}", self.steps, profile_advection.elapsed().as_secs_f64()*1000.);
            }
            if let Some(fluid) = &mut self.local_fluid {
                fluid
                    .advance(''', 1),
    ('        self.steps += 1;', '''        if std::env::var_os("BLOOD_PROFILE").is_some() {
            eprintln!("PROFILE_INNER frame={} inner_ms={:.6}", self.steps, profile_inner.elapsed().as_secs_f64()*1000.);
        }
        self.steps += 1;''', 1),
    ('        self.solve_ms = started.elapsed().as_secs_f64() * 1000.0;', '''        self.solve_ms = started.elapsed().as_secs_f64() * 1000.0;
        #[cfg(feature = "lbm-gpu")]
        if std::env::var_os("BLOOD_PROFILE").is_some() {
            if let (Some(s), Some(w)) = (self.lbm_stats(), self.lbm_work_stats()) {
                eprintln!("PROFILE_STEP frame={} total_ms={:.6} prepare_ms={:.6} coupling_ms={:.6} lbm_ms={:.6} rapier_ms={:.6} upload_ms={:.6} encoding_ms={:.6} submit_ms={:.6} wait_ms={:.6} decode_ms={:.6}", self.steps-1, self.solve_ms, s.prepare_ms, s.coupling_ms, s.lbm_ms, s.rapier_ms, w.upload_ms, w.encoding_ms, w.submission_ms, w.readback_wait_ms, w.readback_decode_ms);
            }
        }''', 1),
])
edit('crates/godot_bridge/src/lib.rs', [
    ('            profile: Self::profile(profile)?,', '            profile: Self::profile(profile)?,\n            allow_software: std::env::var_os("BLOOD_ALLOW_SOFTWARE").is_some(),', 2),
    ('        let state = self.simulation.snapshot();\n        let validation', '''        let profile_start = std::time::Instant::now();
        let state = self.simulation.snapshot();
        let profile_snapshot_ms = profile_start.elapsed().as_secs_f64()*1000.;
        let validation''', 1),
    ('        vdict! {\n            "lbm" => &lbm,', '''        if std::env::var_os("BLOOD_PROFILE").is_some() {
            eprintln!("PROFILE_SNAPSHOT frame={} sim_ms={:.6} before_return_ms={:.6}", state.steps, profile_snapshot_ms, profile_start.elapsed().as_secs_f64()*1000.);
        }
        vdict! {
            "lbm" => &lbm,''', 1),
])
# GPU timestamps are opt-in, resolved together with an existing readback; no new
# queue submission or device poll is added. Timestamp pairs enclose each pass.
edit('crates/fluid-gpu/src/lbm/gpu.rs', [
    ('pub struct LbmWindow {', '''struct ProfileQueries {
    query: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    used: Cell<u32>,
    period: f64,
}
impl ProfileQueries {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let query = device.create_query_set(&wgpu::QuerySetDescriptor { label: Some("profile"), ty: wgpu::QueryType::Timestamp, count: 128 });
        let buffer = |usage| device.create_buffer(&wgpu::BufferDescriptor { label: Some("profile"), size: 1024, usage, mapped_at_creation: false });
        Self { query, resolve: buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC), read: buffer(wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ), used: Cell::new(0), period: queue.get_timestamp_period() as f64 }
    }
    fn writes(&self) -> wgpu::ComputePassTimestampWrites<'_> {
        let i = self.used.get();
        assert!(i+2 <= 128, "profile query capacity exceeded");
        self.used.set(i+2);
        wgpu::ComputePassTimestampWrites { query_set: &self.query, beginning_of_pass_write_index: Some(i), end_of_pass_write_index: Some(i+1) }
    }
    fn resolve(&self, encoder: &mut wgpu::CommandEncoder) {
        let n = self.used.get();
        if n > 0 {
            encoder.resolve_query_set(&self.query, 0..n, &self.resolve, 0);
            encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, u64::from(n)*8);
        }
    }
    fn map(&self) {
        if self.used.get() > 0 {
            self.read.slice(..).map_async(wgpu::MapMode::Read, |r| r.expect("profile map"));
        }
    }
    fn report(&self, kind: &str) {
        let n = self.used.replace(0) as usize;
        if n == 0 { return; }
        let data = self.read.slice(..).get_mapped_range();
        let ticks: &[u64] = bytemuck::cast_slice(&data);
        let sum: u64 = ticks[..n].chunks_exact(2).map(|p| p[1].wrapping_sub(p[0])).sum();
        eprintln!("PROFILE_GPU kind={} passes={} kernels_ms={:.6} span_ms={:.6}", kind, n/2, sum as f64*self.period/1e6, ticks[n-1].wrapping_sub(ticks[0]) as f64*self.period/1e6);
        drop(data);
        self.read.unmap();
    }
}
pub struct LbmWindow {
    profile_queries: Option<ProfileQueries>,''', 1),
    ('        let (device, queue) = pollster::block_on(adapter.request_device', '''        let profiling = std::env::var_os("BLOOD_GPU_PROFILE").is_some();
        let features = if profiling { wgpu::Features::TIMESTAMP_QUERY } else { wgpu::Features::empty() };
        if !adapter.features().contains(features) { return Err("timestamp queries unavailable".into()); }
        let (device, queue) = pollster::block_on(adapter.request_device''', 1),
    ('required_features: wgpu::Features::empty(),', 'required_features: features,', 1),
    ('        Ok(Self {\n            work:', '        let profile_queries = profiling.then(|| ProfileQueries::new(&device, &queue));\n        Ok(Self {\n            profile_queries,\n            work:', 1),
    ('timestamp_writes: None,', 'timestamp_writes: self.profile_queries.as_ref().map(ProfileQueries::writes),', 5),
    ('        self.submit(encoder);\n        let wait_started = Instant::now();', '''        if let Some(q) = &self.profile_queries { q.resolve(&mut encoder); }
        self.submit(encoder);
        let wait_started = Instant::now();
        if let Some(q) = &self.profile_queries { q.map(); }''', 2),
    ('        let decode_started = Instant::now();', '''        if let Some(q) = &self.profile_queries { q.report("readback"); }
        let decode_started = Instant::now();''', 2),
])
edit('godot/world.gd', [
    ('var previous_render_usec := 0', '''var previous_render_usec := 0
var profile_draw_ms := 0.0
var profile_physics_ms := 0.0
var profile_csv_ms := 0.0
var profile_refresh_ms := 0.0
var profile_hud_ms := 0.0
var profile_trace: FileAccess
var profile_visual := "full"''', 1),
    ('\tvar use_lbm := args.has("--local-lbm")', '''\tif OS.has_environment("BLOOD_PROFILE"):
\t\tprofile_trace = FileAccess.open(OS.get_environment("BLOOD_PROFILE") + ".gd.csv", FileAccess.WRITE)
\t\tprofile_trace.store_csv_line(PackedStringArray(["physics_frame", "draw_script_ms", "physics_callback_ms", "physics_csv_ms", "snapshot_bridge_ms", "hud_ms", "process_monitor_ms", "physics_monitor_ms"]))
\t\tprofile_visual = OS.get_environment("BLOOD_PROFILE_VISUAL")
\t\tif profile_visual == "no-hud":
\t\t\thud.hide()
\tvar use_lbm := args.has("--local-lbm")''', 1),
    ('\t\t\t_capture_benchmark_screenshot()\n', '\t\t\t_capture_benchmark_screenshot()\n\tprofile_physics_ms = (Time.get_ticks_usec() - started) / 1000.0\n', 1),
    ('\tprevious_render_usec = started', '''\tprevious_render_usec = started
\tif profile_trace != null:
\t\tprofile_trace.store_csv_line(PackedStringArray([str(benchmark_frame), str(profile_draw_ms), str(profile_physics_ms), str(profile_csv_ms), str(profile_refresh_ms), str(profile_hud_ms), str(Performance.get_monitor(Performance.TIME_PROCESS)*1000.0), str(Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS)*1000.0)]))
\t\tprofile_draw_ms = 0.0''', 1),
    ('func _refresh() -> void:\n\tstate = simulation.snapshot()', '''func _refresh() -> void:
\tvar profile_started := Time.get_ticks_usec()
\tstate = simulation.snapshot()
\tprofile_refresh_ms = (Time.get_ticks_usec() - profile_started) / 1000.0
\tprofile_started = Time.get_ticks_usec()''', 1),
    ('\thud.update_state(state)\n\tqueue_redraw()', '''\tif profile_visual != "no-hud":
\t\thud.update_state(state)
\tprofile_hud_ms = (Time.get_ticks_usec() - profile_started) / 1000.0
\tqueue_redraw()''', 1),
    ('func _draw() -> void:\n', '''func _draw() -> void:
\tvar profile_started := Time.get_ticks_usec()
\tif profile_visual == "no-world":
\t\tprofile_draw_ms = 0.0
\t\treturn
''', 1),
    ('\n\nfunc _cell_centers()', '\n\tprofile_draw_ms = (Time.get_ticks_usec() - profile_started) / 1000.0\n\n\nfunc _cell_centers()', 1),
    ('\tvar l: Dictionary = state.get("lbm", {})', '\tvar profile_started := Time.get_ticks_usec()\n\tvar l: Dictionary = state.get("lbm", {})', 1),
    ('\tbenchmark_frame += 1', '\tprofile_csv_ms = (Time.get_ticks_usec() - profile_started) / 1000.0\n\tbenchmark_frame += 1', 1),
    ('\t\tbenchmark_file.flush()\n\t\trender_file.flush()\n\t\tvar status', '\t\tbenchmark_file.flush()\n\t\trender_file.flush()\n\t\tif profile_trace != null:\n\t\t\tprofile_trace.flush()\n\t\tvar status', 1),
])
print('Instrumented archive:', root)
