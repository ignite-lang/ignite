#!/bin/zsh
# usage: p3-todod-llvm.sh [pairs]
# Interleaved cold LLVM release builds of examples/todo/todo.ignite (docs/slice.md 15.1):
# one on P0's copy of 60aef38 with its ignite, one on the slice-11 tree with P3's ignite,
# alternating; the same release runtime archive for both. Then clang -c -O2 of each
# tree's std.ll alone, three times.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
BASE_TREE=$S/s11-p0-tree
BASE_IGNITE=$S/target-p0/debug/ignite
RT=$S/target-p0/ignite-test/rt/release/libignite_rt.a
NEW_TREE=/home/timo/projects/telos
NEW_IGNITE=$S/target-p3/debug/ignite
OUT=$S/p3-bench
PAIRS=${1:-4}
log=$OUT/todod-llvm.log
mkdir -p $OUT
: > $log
ulimit -c 0
load1() { cut -d' ' -f1 /proc/loadavg }
timed() {
  local label=$1 tree=$2 ignite=$3 out=$4
  rm -rf $out
  local lb=$(load1)
  local t0=$(date +%s.%N)
  (cd $tree && $ignite build examples/todo/todo.ignite --target llvm --release --runtime $RT --out $out > $out.stdout 2>&1)
  local rc=$?
  local t1=$(date +%s.%N)
  local la=$(load1)
  printf 'STEP %s secs=%.3f load_before=%s load_after=%s exit=%s\n' $label $(echo "$t1 - $t0" | bc) $lb $la $rc | tee -a $log
}
echo "START $(date -Is) load=$(cat /proc/loadavg)" | tee -a $log
for i in $(seq 1 $PAIRS); do
  timed base:$i $BASE_TREE $BASE_IGNITE $OUT/base-$i
  timed new:$i $NEW_TREE $NEW_IGNITE $OUT/new-$i
done
for which in base new; do
  ll=$OUT/$which-$PAIRS/llvm/std.ll
  echo "LINES $which std.ll lines=$(wc -l < $ll) defines=$(grep -c '^define' $ll)" | tee -a $log
  for k in 1 2 3; do
    t0=$(date +%s.%N)
    clang -c -O2 -g -ffunction-sections -fdata-sections $ll -o $OUT/$which-std.o
    t1=$(date +%s.%N)
    printf 'CLANG %s std.ll secs=%.3f load=%s\n' $which $(echo "$t1 - $t0" | bc) $(load1) | tee -a $log
  done
done
echo EXIT=0 | tee -a $log
