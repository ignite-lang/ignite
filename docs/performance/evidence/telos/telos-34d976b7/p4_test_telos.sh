#!/bin/zsh
# P4: run the inline Telos tests of std/ and examples/ on both targets.
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_INCREMENTAL=0
cd /home/timo/projects/telos || exit 1
LOG=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/p4_test_telos.log
{
  echo "START $(date +%T)"
  ulimit -c 0
  task test-telos ${FILTER:+FILTER="$FILTER"} 2>&1
  echo "task EXIT=$?"
  echo "END $(date +%T)"
  echo "EXIT=0"
} > "$LOG" 2>&1
