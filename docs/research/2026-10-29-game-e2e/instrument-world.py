#!/usr/bin/env python3
"""Extend an extracted d0dfa07 world.gd for the analytic hold measurement only."""
from pathlib import Path
import sys
p=Path(sys.argv[1])/'godot/world.gd'
s=p.read_text()
def replace(before,after):
 global s
 assert s.count(before)==1,(before,s.count(before))
 s=s.replace(before,after,1)
replace('(not validation or not benchmark in','(not benchmark in')
replace('var target := Vector2(24, 24)','var target := Vector2(-20, 0)')
needle='\tvar q: Dictionary = state.get("validation", {})'
replace(needle,needle+'''
\t# Diagnostic-only analytic-world hold: mark unavailable LBM quality blank.
\tif q.is_empty():
\t\tq = {"active_cells": "", "nearby_cells8": "", "area_ratio_min": "", "area_ratio_max": "", "node_wall_max": "", "robot_wall": "", "finite": true}
\t\tfor cell in state.cells:
\t\t\tfor point in cell:
\t\t\t\tif not point.is_finite():
\t\t\t\t\tq.finite = false
\t\tif not Vector2(state.robot).is_finite() or not q.finite:
\t\t\tq["guard_error"] = "nonfinite analytic-world snapshot"
''')
p.write_text(s)
