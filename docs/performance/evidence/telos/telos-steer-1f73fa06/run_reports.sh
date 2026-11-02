#!/bin/zsh
set -u
cd /home/timo/projects/telos
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-h
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
ulimit -c 0
LOG=/home/timo/.cache/scratch/1f73fa06-e945-4828-ae60-02b7cdacb054/reports.log
{
  cargo test -p telos --test main -- \
    telos_test::assert_and_try_reports \
    telos_test::both_targets_print_the_same_report \
    telos_test::overflow_fails_the_same_everywhere \
    slice9::failed_asserts_print_the_same_debug_text_on_both_targets
  echo "EXIT=$?"
} > "$LOG" 2>&1
