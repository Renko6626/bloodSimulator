#!/usr/bin/env python3
"""Reuse unchanged measurement checks; base=96² and wide=192² in this trial."""
import hashlib
import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PREV = ROOT.parent / '2026-10-27-wide-window'
SPEC = importlib.util.spec_from_file_location('wide_analysis', PREV / 'analyze.py')
ANALYSIS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ANALYSIS)
ANALYSIS.ROOT = ROOT
ANALYSIS.REPO = ROOT.parents[3]


if __name__ == '__main__':
    ANALYSIS.main()
