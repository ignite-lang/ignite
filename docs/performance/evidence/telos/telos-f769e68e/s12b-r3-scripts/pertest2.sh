#!/bin/zsh
# usage: pertest2.sh <pass>
# One interleaved pass over three trees: before (s12-before, 11b), s12 (s12-p4-after2, the
# slice-12 close) and work (the working tree with the cut ignite_test.rs, target-12b-r3).
# Units: the warm ignite_test.rs file on each tree; each of its tests alone on s12 and work;
# the other end-to-end files slice 12 added on s12 and work. STEP lines as env.sh's timed.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p4-scripts/env.sh
LOGS=$S/s12b-r3-logs
mkdir -p $LOGS
pass=$1
log=$LOGS/pertest2-$pass.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) pass=$pass load=$(cat /proc/loadavg) free=$(free_gb)G" | tee -a "$log.summary"

tree_of() {
  case $1 in
    before) cd $S/s12-before || exit 97; export CARGO_TARGET_DIR=$S/target-s12-before ;;
    s12)    cd $S/s12-p4-after2 || exit 97; export CARGO_TARGET_DIR=$S/target-s12-p4-after2 ;;
    work)   cd /home/timo/projects/telos || exit 97; export CARGO_TARGET_DIR=$S/target-12b-r3 ;;
  esac
}

for tree in before s12 work; do
  tree_of $tree
  timed "$log" $tree no-run $pass cargo test -p ignite --test main --no-run
done
for tree in before s12 work; do
  tree_of $tree
  timed "$log" $tree file-ignite_test $pass cargo test -p ignite --test main "ignite_test::"
done

tree_of work
work_tests=$(cargo test -q -p ignite --test main "ignite_test::" -- --list 2>/dev/null | sed -n 's/^ignite_test::\(.*\): test$/\1/p')
tree_of s12
s12_tests=$(cargo test -q -p ignite --test main "ignite_test::" -- --list 2>/dev/null | sed -n 's/^ignite_test::\(.*\): test$/\1/p')
all_tests=$(print -l ${(f)work_tests} ${(f)s12_tests} | sort -u)
echo "TESTS $(echo $all_tests | tr '\n' ' ')" | tee -a "$log.summary"
for t in ${(f)all_tests}; do
  for tree in s12 work; do
    tree_of $tree
    grep -q "^fn $t()" crates/ignite/tests/ignite_test.rs || continue
    timed "$log" $tree "test-$t" $pass cargo test -p ignite --test main "ignite_test::$t" -- --exact
  done
done

for f in run drain control_trace decode; do
  for tree in s12 work; do
    tree_of $tree
    timed "$log" $tree "file-$f" $pass cargo test -p ignite --test main "$f::"
  done
done
finish "$log"
exit $FAILED
