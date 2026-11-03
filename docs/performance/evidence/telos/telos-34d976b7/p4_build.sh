#!/bin/zsh
# P4: build the telos CLI and the runner runtime into the scratch target dir.
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_INCREMENTAL=0
cd /home/timo/projects/telos || exit 1
LOG=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/p4_build.log
{
  echo "START $(date +%T)"
  cargo build -p telos 2>&1
  echo "telos EXIT=$?"
  cargo build -p telos-rt --features runner --target-dir "$CARGO_TARGET_DIR/telos-test/rt" 2>&1
  echo "rt EXIT=$?"
  echo "END $(date +%T)"
  echo "EXIT=0"
} > "$LOG" 2>&1
