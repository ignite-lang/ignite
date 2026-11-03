#!/bin/zsh
# usage: s9b-norun-ab.sh   cargo test --no-run after cargo clean, before and after trees
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
now() { date +%s.%N }
for side in before after; do
  cd $S/s9b-$side-tree
  export CARGO_TARGET_DIR=$S/target-k-$side
  cargo clean > /dev/null 2>&1
  t0=$(now)
  cargo test --workspace --no-run > $S/s9b-norun-$side.log 2>&1
  r=$?
  t1=$(now)
  touch crates/telos-check/src/lib.rs
  cargo test --workspace --no-run > /dev/null 2>&1
  t2=$(now)
  printf '%s cached=%.1f(exit %s) touch=%.1f load=%s\n' $side $(( t1 - t0 )) $r $(( t2 - t1 )) "$(cut -d' ' -f1 /proc/loadavg)"
done
