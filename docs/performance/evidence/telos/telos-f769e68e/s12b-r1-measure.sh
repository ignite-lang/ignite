#!/bin/zsh
# usage: s12b-r1-measure.sh <tag>
# Interleaved before/after: ignite check and ignite plan of the design file (15 pairs each),
# todod LLVM dev build (3 pairs). before = s12-before tree and binary; after = the working tree
# with the r1 binary. STEP lines in the P4 format, summarised by pairs.py.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
tag=$1
LOG=$S/s12b-r1-measure-$tag.log
SUM=$LOG.summary
OUT=$S/s12b-r1-out-$tag
mkdir -p $OUT
: >$LOG; : >$SUM
ulimit -c 0
DESIGN=examples/terminal/terminal.design.ignite
TODO=examples/todo/todo.ignite
load1() { cut -d' ' -f1 /proc/loadavg; }
use_tree() {
  case $1 in
    before) TREE=$S/s12-before; IGNITE=$S/target-s12-before/debug/ignite; RT=$S/target-s12-before/ignite-test/rt/debug/libignite_rt.a ;;
    after)  TREE=/home/timo/projects/telos; IGNITE=$S/target-12b-r1/debug/ignite; RT=$S/target-12b-r1/debug/libignite_rt.a ;;
  esac
  cd $TREE || exit 97
}
timed() {
  local tree=$1 label=$2 pair=$3; shift 3
  local lb la t0 t1 rc dt
  lb=$(load1)
  t0=$(date +%s.%N)
  "$@" >>$LOG 2>&1
  rc=$?
  t1=$(date +%s.%N)
  la=$(load1)
  dt=$(echo "$t1 - $t0" | bc)
  printf 'STEP tree=%s label=%s pair=%s secs=%.3f load_before=%s load_after=%s exit=%s\n' "$tree" "$label" "$pair" "$dt" "$lb" "$la" "$rc" | tee -a $SUM
}
echo "START $(date -Is) load=$(cat /proc/loadavg)" | tee -a $SUM
for rep in $(seq 1 15); do
  for tree in before after; do
    use_tree $tree
    timed $tree check-design $rep $IGNITE check $DESIGN
  done
  for tree in before after; do
    use_tree $tree
    timed $tree plan-design $rep $IGNITE plan $DESIGN --out $OUT/plan-$tree-$rep.json
  done
done
for rep in 1 2 3; do
  for tree in before after; do
    use_tree $tree
    out=$OUT/todod-llvm-$tree-$rep
    mkdir -p $out
    timed $tree todod-llvm-dev $rep $IGNITE build $TODO --target llvm --runtime $RT --out $out
  done
done
for rep in 1 2 3; do
  for tree in before after; do
    use_tree $tree
    timed $tree check-todo $rep $IGNITE check $TODO
  done
done
cmp $OUT/plan-before-1.json $OUT/plan-after-1.json && echo "PLAN JSON before==after" | tee -a $SUM || echo "PLAN JSON DIFFERS" | tee -a $SUM
python3 -I $S/s12-p4-scripts/pairs.py $SUM >$S/s12b-r1-pairs-$tag.txt 2>&1
echo "EXIT=0 $(date -Is)" | tee -a $SUM
