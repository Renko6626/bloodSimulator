#!/usr/bin/env python3
"""Add callback boundary clocks to an already instrumented extracted tree."""
from pathlib import Path
import sys
root=Path(sys.argv[1]); assert not (root/'.git').exists()
p=root/'godot/world.gd';s=p.read_text()
def replace(a,b):
    global s
    assert s.count(a)==1,(a,s.count(a))
    s=s.replace(a,b,1)
replace('var profile_physics_ms := 0.0','var profile_physics_start_usec := 0\nvar profile_physics_end_usec := 0\nvar profile_render_end_usec := 0\nvar profile_physics_ms := 0.0')
replace('func _physics_process(_delta: float) -> void:\n\tvar started := Time.get_ticks_usec()',
        'func _physics_process(_delta: float) -> void:\n\tvar started := Time.get_ticks_usec()\n\tprofile_physics_start_usec = started')
replace('\tprofile_physics_ms = (Time.get_ticks_usec() - started) / 1000.0',
        '\tprofile_physics_end_usec = Time.get_ticks_usec()\n\tprofile_physics_ms = (profile_physics_end_usec - started) / 1000.0')
replace('"draw_end_ago_ms"]))','"draw_end_ago_ms", "after_physics_to_process_ms", "after_render_to_physics_ms"]))')
replace('str((started-profile_draw_end_usec)/1000.0)]))',
        'str((started-profile_draw_end_usec)/1000.0), str((started-profile_physics_end_usec)/1000.0), str((profile_physics_start_usec-profile_render_end_usec)/1000.0)]))')
replace('\tprofile_render_span_ms = (Time.get_ticks_usec() - profile_render_start_usec) / 1000.0',
        '\tprofile_render_end_usec = Time.get_ticks_usec()\n\tprofile_render_span_ms = (profile_render_end_usec - profile_render_start_usec) / 1000.0')
p.write_text(s)
