#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all --check
cargo check --locked --workspace
cargo test --locked -p simulation
bash scripts/build.sh
"${GODOT_BIN:-godot}" --headless --path godot --editor --import --quit
"${GODOT_BIN:-godot}" --headless --path godot --fixed-fps 60 --quit-after 180

"${GODOT_BIN:-godot}" --headless --path godot --fixed-fps 60 res://lab.tscn --quit-after 180
