#!/bin/zsh
# P4: build the audit's iotest harness in its own target dir and run drop and cross (14.0.4).
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/iotest-target
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_INCREMENTAL=0
LOG=$S/p4_iotest.log
{
  echo "START $(date +%T)"
  cargo build --manifest-path /home/timo/projects/telos/docs/audit/2026-11-01/io/iotest/Cargo.toml 2>&1
  echo "build EXIT=$?"
  echo "--- drop"
  timeout 30 $CARGO_TARGET_DIR/debug/iotest drop 2>&1
  echo "drop EXIT=$?"
  echo "--- cross"
  timeout 30 $CARGO_TARGET_DIR/debug/iotest cross 2>&1
  echo "cross EXIT=$?"
  echo "END $(date +%T)"
  echo "EXIT=0"
} > "$LOG" 2>&1
