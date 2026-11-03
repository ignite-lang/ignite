#!/bin/zsh
# usage: pertest.sh <pass> [tree...]
# Warm per-test timing of crates/ignite/tests/ignite_test.rs on the before tree and the after2
# snapshot (interleaved): each test alone with --exact, then the whole file, then the other
# end-to-end files slice 12 added (run, drain, control_trace, decode). No touch: the test binary
# is warm, so the time is test execution.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p4-scripts/env.sh
LOGS=$S/s12b-r3-logs
mkdir -p $LOGS
pass=$1; shift
trees=(before after2)
[[ $# -gt 0 ]] && trees=("$@")
log=$LOGS/pertest-$pass.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) pass=$pass trees=$trees load=$(cat /proc/loadavg) free=$(free_gb)G" | tee -a "$log.summary"

for tree in $trees; do
  use_tree $tree
  timed "$log" $tree no-run-warm $pass cargo test -p ignite --test main --no-run
done

use_tree ${trees[-1]}
tests=$(cargo test -q -p ignite --test main "ignite_test::" -- --list 2>/dev/null | sed -n 's/^ignite_test::\(.*\): test$/\1/p')
echo "TESTS $(echo $tests | tr '\n' ' ')" | tee -a "$log.summary"

for t in ${(f)tests}; do
  for tree in $trees; do
    use_tree $tree
    grep -q "^fn $t()" crates/ignite/tests/ignite_test.rs || continue
    timed "$log" $tree "test-$t" $pass cargo test -p ignite --test main "ignite_test::$t" -- --exact
  done
done

for f in ignite_test run drain control_trace decode; do
  for tree in $trees; do
    use_tree $tree
    [[ -f crates/ignite/tests/$f.rs ]] || continue
    timed "$log" $tree "file-$f" $pass cargo test -p ignite --test main "$f::"
  done
done
finish "$log"
echo "EXIT=$FAILED" >>"$log.summary"
exit $FAILED
