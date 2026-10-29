#!/usr/bin/env python3
"""Instrument an extracted d0dfa07 tree. Product checkout is never edited."""
from pathlib import Path
import subprocess,sys
root=Path(sys.argv[1]).resolve()
assert not (root/'.git').exists()
old=Path(__file__).resolve().parent.parent/'2026-10-29-gameplay-profile/instrument.py'
s=old.read_text()
s=s.replace('        vdict! {\\n            "lbm" => &lbm,','        vdict! {\\n            "population" => &population,\\n            "lbm" => &lbm,')
s=s.replace('        vdict! {\n            "lbm" => &lbm,','        vdict! {\n            "population" => &population,\n            "lbm" => &lbm,')
base=root/'.diagnostic-base.py';base.write_text(s)
subprocess.run([sys.executable,str(base),str(root)],check=True)
def patch(path,before,after):
 p=root/path;s=p.read_text();assert s.count(before)==1,(path,before[:80],s.count(before));p.write_text(s.replace(before,after,1))
# Establish a frame boundary before any instrumented inner/GPU operations.
patch('crates/simulation/src/runtime.rs',
 '        let started = Instant::now();\n        let result = crate::threading::install',
 '''        let started = Instant::now();
        if std::env::var_os("BLOOD_PROFILE").is_some() { eprintln!("PROFILE_BEGIN frame={}", self.steps); }
        let result = crate::threading::install''')
# Separate the three existing reads; preserve the original poll and submission count.
p=root/'crates/fluid-gpu/src/lbm/gpu.rs';s=p.read_text()
for begin,end,kind in [('    fn populations(&self)', '    pub fn control_mean_density', 'raw'),
                       ('    pub fn field(&self)', '    pub fn sample(', 'field')]:
 a=s.index(begin);b=s.index(end,a);part=s[a:b]
 before='        self.record(|s| s.readback_wait_ms += wait_started.elapsed().as_secs_f64() * 1000.);'
 assert part.count(before)==1
 part=part.replace(before,'''        let profile_wait_ms = wait_started.elapsed().as_secs_f64() * 1000.;
        self.record(|s| s.readback_wait_ms += profile_wait_ms);''')
 before='        self.record(|s| s.readback_decode_ms += decode_started.elapsed().as_secs_f64() * 1000.);'
 assert part.count(before)==1
 part=part.replace(before,'''        let profile_decode_ms = decode_started.elapsed().as_secs_f64() * 1000.;
        self.record(|s| s.readback_decode_ms += profile_decode_ms);
        if std::env::var_os("BLOOD_PROFILE").is_some() {
            eprintln!("PROFILE_READBACK kind='''+kind+''' wait_ms={profile_wait_ms:.6} decode_ms={profile_decode_ms:.6}");
        }''')
 s=s[:a]+part+s[b:]
p.write_text(s)
patch('crates/fluid-gpu/src/lbm/gpu.rs','        let raw = self.populations()?;',
 '        let raw = self.populations()?;\n        let profile_reduce = Instant::now();')
patch('crates/fluid-gpu/src/lbm/gpu.rs','        self.remove_mean_offset(delta);',
 '''        let profile_reduce_ms = profile_reduce.elapsed().as_secs_f64()*1000.;
        let profile_correct = Instant::now();
        self.remove_mean_offset(delta);
        if std::env::var_os("BLOOD_PROFILE").is_some() {
            eprintln!("PROFILE_MEAN reduce_ms={profile_reduce_ms:.6} correction_submit_ms={:.6}",profile_correct.elapsed().as_secs_f64()*1000.);
        }''')
patch('crates/simulation/src/fluid/streaming_window.rs','''    fn refresh_field(&mut self) -> Result<(), String> {
        let field = self.window.field()?;''','''    fn refresh_field(&mut self) -> Result<(), String> {
        let profile_read = std::time::Instant::now();
        let field = self.window.field()?;
        let profile_read_ms = profile_read.elapsed().as_secs_f64()*1000.;
        let profile_host = std::time::Instant::now();''')
patch('crates/simulation/src/fluid/streaming_window.rs','''        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn diagnostic_momentum''','''        if std::env::var_os("BLOOD_PROFILE").is_some() {
            eprintln!("PROFILE_CACHE read_ms={profile_read_ms:.6} host_ms={:.6}",profile_host.elapsed().as_secs_f64()*1000.);
        }
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn diagnostic_momentum''')
# Wall spans plus draw timestamps identify the actual callback order.
p=root/'godot/world.gd';s=p.read_text()
s=s.replace('var profile_trace: FileAccess', '''var profile_process_prev_ms := 0.0
var profile_process_end_usec := 0
var profile_pre_gap_ms := 0.0
var profile_render_start_usec := 0
var profile_render_span_ms := 0.0
var profile_render_pre_frame := -1
var profile_render_post_frame := -1
var profile_render_gpu := false
var profile_trace: FileAccess''',1)
s=s.replace('"physics_monitor_ms"]))','"physics_monitor_ms", "process_callback_prev_ms", "pre_render_prev_ms", "render_pipeline_prev_ms", "render_pre_frame", "render_post_frame", "renderer_setup_cpu_prev_ms", "viewport_cpu_cached_ms", "viewport_gpu_cached_ms", "process_frame"]))',1)
s=s.replace('\t\tprofile_visual = OS.get_environment("BLOOD_PROFILE_VISUAL")','''\t\tRenderingServer.frame_pre_draw.connect(_profile_render_pre)
\t\tRenderingServer.frame_post_draw.connect(_profile_render_post)
\t\tprofile_render_gpu = OS.has_environment("BLOOD_GODOT_GPU_PROFILE")
\t\tRenderingServer.viewport_set_measure_render_time(get_viewport().get_viewport_rid(), profile_render_gpu)
\t\tprofile_visual = OS.get_environment("BLOOD_PROFILE_VISUAL")''',1)
s=s.replace('str(Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS)*1000.0)]))', '''str(Performance.get_monitor(Performance.TIME_PHYSICS_PROCESS)*1000.0), str(profile_process_prev_ms), str(profile_pre_gap_ms), str(profile_render_span_ms), str(profile_render_pre_frame), str(profile_render_post_frame), str(RenderingServer.get_frame_setup_time_cpu()), str(RenderingServer.viewport_get_measured_render_time_cpu(get_viewport().get_viewport_rid()) if profile_render_gpu else -1.0), str(RenderingServer.viewport_get_measured_render_time_gpu(get_viewport().get_viewport_rid()) if profile_render_gpu else -1.0), str(Engine.get_process_frames())]))''',1)
a=s.index('func _process(');b=s.index('\n\nfunc _movement()',a)
s=s[:b]+'''\n\tprofile_process_end_usec = Time.get_ticks_usec()
\tprofile_process_prev_ms = (profile_process_end_usec - started) / 1000.0
'''+s[b:]
s+='''

func _profile_render_pre() -> void:
\tprofile_render_start_usec = Time.get_ticks_usec()
\tprofile_render_pre_frame = benchmark_frame
\tprofile_pre_gap_ms = (profile_render_start_usec - profile_process_end_usec) / 1000.0


func _profile_render_post() -> void:
\tprofile_render_span_ms = (Time.get_ticks_usec() - profile_render_start_usec) / 1000.0
\tprofile_render_post_frame = benchmark_frame
'''
s=s.replace('var profile_process_prev_ms :=', 'var profile_draw_end_usec := 0\nvar profile_draw_start_usec := 0\nvar profile_process_prev_ms :=',1)
s=s.replace('"process_frame"]))', '"process_frame", "draw_start_ago_ms", "draw_end_ago_ms"]))',1)
s=s.replace('str(Engine.get_process_frames())]))', 'str(Engine.get_process_frames()), str((started-profile_draw_start_usec)/1000.0), str((started-profile_draw_end_usec)/1000.0)]))',1)
s=s.replace('func _draw() -> void:\n\tvar profile_started := Time.get_ticks_usec()', 'func _draw() -> void:\n\tvar profile_started := Time.get_ticks_usec()\n\tprofile_draw_start_usec = profile_started',1)
s=s.replace('\tprofile_draw_ms = (Time.get_ticks_usec() - profile_started) / 1000.0', '\tprofile_draw_end_usec = Time.get_ticks_usec()\n\tprofile_draw_ms = (profile_draw_end_usec - profile_started) / 1000.0',1)
p.write_text(s)
print('Detailed diagnostics applied:',root)
