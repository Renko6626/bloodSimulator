#!/usr/bin/env python3
"""Add a diagnostic control to a copy already patched by instrument.py."""
import sys
from pathlib import Path
p = Path(sys.argv[1]) / 'crates/simulation/examples/lbm_gameplay_probe.rs'
s = p.read_text()
before = '    let mut config = fixture::config();'
assert s.count(before) == 1
p.write_text(s.replace(before, '''    let mut config = fixture::config();
    if args.iter().any(|s| s == "--uncoupled") {
        config.cell_count = 0;
        config.cell_drag = 0.; // The existing exchange scales both cell and robot kicks by drag.
        config.robot_flow = 0.;
    }''', 1))
