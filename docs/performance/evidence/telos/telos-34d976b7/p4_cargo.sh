#!/bin/zsh
# P4: run one cargo command in the workspace; usage: p4_cargo.sh LOGNAME ARGS...
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_INCREMENTAL=0
cd /home/timo/projects/telos || exit 1
LOG=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/$1.log
shift
{
  echo "START $(date +%T)"
  ulimit -c 0
  cargo "$@" 2>&1
  echo "cargo EXIT=$?"
  echo "END $(date +%T)"
  echo "EXIT=0"
} > "$LOG" 2>&1
