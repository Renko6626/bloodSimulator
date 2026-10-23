#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
bash scripts/build.sh
exec "${GODOT_BIN:-godot}" --path godot "$@"
