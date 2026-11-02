#!/bin/zsh
# usage: s9c_build.sh LOG  -- builds telos and the runner runtime into target-h
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-h CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0
cd /home/timo/projects/telos
{
  cargo build -q -p telos && cargo build -q -p telos-rt --features runner --target-dir $S/target-h/rt
  echo "EXIT=$?"
} > $1 2>&1
