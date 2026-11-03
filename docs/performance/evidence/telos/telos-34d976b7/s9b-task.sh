#!/bin/zsh
# usage: s9b-task.sh <outfile> <task args...>
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
OUT=$1
shift
cd /home/timo/projects/telos
export CARGO_TARGET_DIR=$S/target-k CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
{
  t0=$(date +%s.%N)
  task "$@" 2>&1
  rc=$?
  t1=$(date +%s.%N)
  printf 'WALL=%.1f\n' $(( t1 - t0 ))
  echo "EXIT=$rc"
} > $OUT 2>&1
