#!/usr/bin/env bash
set -u
run_dir=/data/sunyunbo/.codex/worktrees/7c02/bloodSimulator/docs/research/2026-11-03-organ-heart-flow/attempt-03
date -u '+start=%Y-%m-%dT%H:%M:%SZ' > "$run_dir/lifecycle.log"
trap 'date -u "+shell_exit=%Y-%m-%dT%H:%M:%SZ" >> "$run_dir/lifecycle.log"' EXIT
trap 'date -u "+shell_sigterm=%Y-%m-%dT%H:%M:%SZ" >> "$run_dir/lifecycle.log"; exit 143' TERM
printf 'Heart bake attempt 03 started; maximum 1200 seconds.\n'
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.x86_64.json \
  timeout --verbose --signal=TERM --kill-after=10s 1200s \
  /data/sunyunbo/www/bloodSimulator/target/release/examples/organ_heart_bake \
  --bake --allow-software --output "$run_dir" > "$run_dir/run.log" 2>&1
bake_status=$?
printf 'command_exit=%s\n' "$bake_status" >> "$run_dir/lifecycle.log"
date -u '+command_end=%Y-%m-%dT%H:%M:%SZ' >> "$run_dir/lifecycle.log"
printf 'Heart bake attempt 03 finished with exit %s.\n' "$bake_status"
exit "$bake_status"
