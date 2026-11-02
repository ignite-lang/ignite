#!/bin/zsh
# usage: s9c_cargo.sh LOG cargo-args...   (env passes through, e.g. TELOS_BLESS)
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-h CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
cd /home/timo/projects/telos
LOG=$1; shift
{
  cargo "$@"
  echo "EXIT=$?"
} > $LOG 2>&1
