#!/bin/zsh
# usage: s9c_tt.sh LOG TARGET FILE [extra args...]
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-h CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
cd /home/timo/projects/telos
LOG=$1; T=$2; shift 2
{
  if [[ $T == llvm ]]; then
    $S/target-h/debug/telos test "$@" --target llvm --out $S/s9c-tt --runtime $S/target-h/rt/debug/libtelos_rt.a
  else
    $S/target-h/debug/telos test "$@" --out $S/s9c-tt --target-dir $S/s9c-tt/cargo
  fi
  echo "EXIT=$?"
} > $LOG 2>&1
