#!/bin/zsh
# usage: invocations.sh <tree> : the cost of one `ignite test` invocation on each target, from
# the named snapshot's driver: cold, warm same program, --list, program switch, compile error,
# the 1 s timeout, overflow dev and release; plus `ignite check` of the fixture as the frontend
# share. Output: STEP lines with seconds and load.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
tree=$1
case $tree in
  before) TREE=$S/s12-before; TD=$S/target-s12-before ;;
  after2) TREE=$S/s12-p4-after2; TD=$S/target-s12-p4-after2 ;;
  *) echo "unknown tree $tree"; exit 98 ;;
esac
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
IGNITE=$TD/debug/ignite
RT=$(grep -l ignite_rt_test_main $TD/debug/deps/libignite_rt-*.a | head -1)
FIX=$TREE/crates/ignite/tests/fixtures/tests
OUT=$S/s12b-r3-build/inv-$tree-$(date +%H%M%S)
mkdir -p $OUT
LOG=$S/s12b-r3-logs/invocations-$tree.log
: >$LOG
echo "START $(date -Is) tree=$tree rt=$RT out=$OUT load=$(cat /proc/loadavg)" | tee -a $LOG
cd $TREE || exit 97
t() {
  local label=$1; shift
  local lb t0 t1 rc
  lb=$(cut -d' ' -f1 /proc/loadavg)
  t0=$(date +%s.%N)
  "$@" >$OUT/$label.out 2>&1
  rc=$?
  t1=$(date +%s.%N)
  printf 'STEP tree=%s label=%s secs=%.2f load=%s exit=%s\n' $tree $label $(echo "$t1 - $t0" | bc) $lb $rc | tee -a $LOG
}
for target in rust llvm; do
  common=(--target $target --out $OUT/$target --runtime $RT)
  [[ $target == rust ]] && common+=(--target-dir $OUT/cargo)
  t $target-check-basics $IGNITE check $FIX/basics.ignite
  t $target-check-basics-2 $IGNITE check $FIX/basics.ignite
  t $target-basics-cold $IGNITE test $FIX/basics.ignite $common
  t $target-basics-warm-1 $IGNITE test $FIX/basics.ignite $common
  t $target-basics-warm-2 $IGNITE test $FIX/basics.ignite $common
  t $target-basics-list $IGNITE test $FIX/basics.ignite differs --list $common
  t $target-basics-exact $IGNITE test $FIX/basics.ignite "fixtures.basics/int differs" --exact $common
  t $target-basics-none $IGNITE test $FIX/basics.ignite "no such test" $common
  t $target-targets-switch $IGNITE test $FIX/targets.ignite $common
  t $target-basics-switchback $IGNITE test $FIX/basics.ignite $common
  t $target-targets-switch-2 $IGNITE test $FIX/targets.ignite $common
  t $target-broken $IGNITE test $FIX/broken.ignite $common
  t $target-slow-timeout1 $IGNITE test $FIX/slow.ignite --timeout 1 $common
  t $target-slow-basics-list $IGNITE test $FIX/slow.ignite $FIX/basics.ignite fa --list $common
  t $target-unnamed-1 $IGNITE test $FIX/unnamed_a.ignite $FIX/unnamed_b.ignite $common
  t $target-unnamed-2 $IGNITE test $FIX/unnamed_a.ignite $FIX/unnamed_b.ignite $common
  t $target-std-greet $IGNITE test $FIX/std/greet.ignite --std $FIX/std $common
  t $target-overflow-dev $IGNITE test $FIX/overflow.ignite $common
  t $target-overflow-release-cold $IGNITE test $FIX/overflow.ignite --release $common
  t $target-overflow-release-warm $IGNITE test $FIX/overflow.ignite --release $common
  if [[ -f $FIX/builtins.ignite ]]; then
    t $target-builtins-test $IGNITE test $FIX/builtins.ignite $common
    t $target-targets-builtins $IGNITE test $FIX/targets.ignite $FIX/builtins.ignite $common
    build=(build $FIX/builtins.ignite --target $target --out $OUT/$target-build)
    [[ $target == rust ]] && build+=(--target-dir $OUT/cargo)
    [[ $target == llvm ]] && build+=(--runtime $RT)
    t $target-builtins-build-cold $IGNITE $build
    t $target-builtins-build-warm $IGNITE $build
  fi
done
echo "EXIT=0 $(date -Is) load=$(cat /proc/loadavg)" | tee -a $LOG
