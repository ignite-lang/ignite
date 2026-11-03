#!/bin/zsh
# usage: s9b-tt-ab.sh <round>   cold telos test runs of the before and after trees into fresh dirs
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
R=$1
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only SCCACHE_RECACHE=1
now() { date +%s.%N }
for side in before after; do
  cd $S/s9b-$side-tree
  T=$S/target-k-$side
  W=$S/s9b-ab/$side-$R
  mkdir -p $W
  FILES=($(find std examples -name '*.telos' ! -name '*.design.telos' | sort))
  t0=$(now)
  $T/debug/telos test $FILES --out $W --target-dir $W/cargo > $W/rust.log 2>&1
  r=$?
  t1=$(now)
  $T/debug/telos test $FILES --target llvm --out $W --runtime $T/telos-test/rt/debug/libtelos_rt.a > $W/llvm.log 2>&1
  l=$?
  t2=$(now)
  printf '%s round=%s rust=%.1f(exit %s) llvm=%.1f(exit %s) execs=%s load=%s\n' $side $R $(( t1 - t0 )) $r $(( t2 - t1 )) $l "$(find $W -name telos-test -type f -perm -u+x | grep -v /cargo/ | wc -l)" "$(cut -d' ' -f1 /proc/loadavg)"
done
