#!/usr/bin/env bash
set -euo pipefail
cd /workspace/blood-game-e2e
export DISPLAY=:99 XDG_RUNTIME_DIR=/tmp/blood-e2e-runtime
export LD_LIBRARY_PATH="$PWD/display/glvnd/usr/lib/x86_64-linux-gnu:$PWD/display/driver:${LD_LIBRARY_PATH:-}"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
for project in baseline current world; do
    # Preserve the known first empty-cache import failure, retry cached import once.
    set +e
    ./godot-bin --headless --path "$project/godot" --editor --import --quit > "$project.import-first.log" 2>&1
    code=$?
    set -e
    printf '%s\n' "$code" > "$project.import-first.exit.txt"
    if [[ "$code" != 0 ]]; then
        ./godot-bin --headless --path "$project/godot" --editor --import --quit > "$project.import-cached.log" 2>&1
    fi
done
printf 'SETUP_COMPLETE\n'
