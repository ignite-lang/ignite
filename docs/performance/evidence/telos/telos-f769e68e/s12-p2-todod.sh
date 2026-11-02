#!/bin/zsh
# usage: s12-p2-todod.sh <label> [pairs]
# Invariant 9 (docs/slice.md 16.0 item 12, 15.1): cold builds of examples/todo/todo.ignite,
# Rust and LLVM targets, dev and release, one on the slice-12 start tree (s12-before) with
# its own ignite and runtime archives, one on the working tree with P2's, alternating.
# The 1-minute load is recorded before and after every build. The log ends with EXIT=0.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
LABEL=${1:-run}
PAIRS=${2:-3}
BASE_TREE=$S/s12-before
BASE_T=$S/target-s12-before-p2
NEW_TREE=/home/timo/projects/telos
NEW_T=$S/target-s12-p2
OUT=$S/s12-p2-bench-$LABEL
log=$OUT/todod.log
mkdir -p $OUT
: > $log
ulimit -c 0
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_RELEASE_DEBUG=0
export SCCACHE_RECACHE=1
load1() { cut -d' ' -f1 /proc/loadavg }
timed() {
  local label=$1 tree=$2 ignite=$3 out=$4
  shift 4
  rm -rf $out
  mkdir -p $out
  local lb=$(load1)
  local t0=$(date +%s.%N)
  (cd $tree && $ignite build examples/todo/todo.ignite --out $out/project --target-dir $out/target "$@" > $out/build.stdout 2>&1)
  local rc=$?
  local t1=$(date +%s.%N)
  local la=$(load1)
  printf 'STEP %s secs=%.3f load_before=%s load_after=%s exit=%s\n' $label $(echo "$t1 - $t0" | bc) $lb $la $rc | tee -a $log
}
echo "START $(date -Is) load=$(cat /proc/loadavg)" | tee -a $log
for i in $(seq 1 $PAIRS); do
  for which in base new; do
    if [ $which = base ]; then tree=$BASE_TREE; t=$BASE_T; else tree=$NEW_TREE; t=$NEW_T; fi
    timed ${which}:rust-dev:${i} $tree $t/debug/ignite $OUT/$which-rust-dev-$i
    timed ${which}:rust-release:${i} $tree $t/debug/ignite $OUT/$which-rust-release-$i --release
    timed ${which}:llvm-dev:${i} $tree $t/debug/ignite $OUT/$which-llvm-dev-$i --target llvm --runtime $t/debug/libignite_rt.a
    timed ${which}:llvm-release:${i} $tree $t/debug/ignite $OUT/$which-llvm-release-$i --target llvm --release --runtime $t/release/libignite_rt.a
  done
done
echo "EXIT=0" | tee -a $log
