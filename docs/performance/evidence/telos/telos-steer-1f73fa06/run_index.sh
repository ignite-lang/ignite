#!/bin/zsh
set -u
cd /home/timo/projects/telos
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-h
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
ulimit -c 0
LOG=/home/timo/.cache/scratch/1f73fa06-e945-4828-ae60-02b7cdacb054/index.log
{
  cargo test -p telos --test main -- llvm::index_out_of_range_panics_with_101 --nocapture
  echo "EXIT=$?"
} > "$LOG" 2>&1
