#!/bin/zsh
# usage: invocation.sh <reps>
# Cost of one warm `ignite test` subprocess as slice9.rs and ignite_test.rs run it (same out and
# target dirs as the e2e tests, so cargo and the clang stamps are warm), per target, timed.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p0-scripts/env.sh
reps=${1:-3}
log=$LOGS/invocation.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) load=$(cat /proc/loadavg)" | tee -a "$log.summary"
IGNITE=$CARGO_TARGET_DIR/debug/ignite
TMP=$CARGO_TARGET_DIR/tmp
RT=$(ls -t $CARGO_TARGET_DIR/debug/deps/libignite_rt-*.a | head -1)
echo "runtime=$RT" >>"$log"
for rep in $(seq 1 $reps); do
  for target in rust llvm; do
    timed "$log" "slice9-audit-index_if:$target:$rep" $IGNITE test docs/audit/2026-11-01/llvm/index_if.ignite --target $target --out $TMP/ignite-slice9/audit --runtime $RT --target-dir $TMP/ignite-slice9/audit/cargo
    timed "$log" "ignite_test-basics:$target:$rep" $IGNITE test crates/ignite/tests/fixtures/tests/basics.ignite --target $target --out $TMP/ignite-test --runtime $RT --target-dir $TMP/ignite-test/cargo
    timed "$log" "slice9-regions:$target:$rep" $IGNITE test crates/ignite/tests/fixtures/slice9/regions.ignite --target $target --out $TMP/ignite-slice9/regions --runtime $RT --target-dir $TMP/ignite-slice9/regions/cargo
  done
done
for rep in $(seq 1 $reps); do
  timed "$log" "cargo-noop-in-audit-target-dir:$rep" cargo build -q --manifest-path $TMP/ignite-slice9/audit/Cargo.toml --target-dir $TMP/ignite-slice9/audit/cargo
done
ls $TMP/ignite-slice9/audit >>"$log" 2>&1
ls $TMP/ignite-test >>"$log" 2>&1
echo "EXIT=0" | tee -a "$log.summary"
