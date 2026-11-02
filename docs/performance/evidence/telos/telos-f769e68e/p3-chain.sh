#!/bin/zsh
# P3 sequential chain: fmt check, clippy, LLVM todod measurement, 30 failover runs alone.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
cd /home/timo/projects/telos || exit 9
export CARGO_TARGET_DIR=$S/target-p3 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0
ulimit -c 0
log=$S/p3-chain.log
: > $log
step() { echo "STEP $1 $(date -Is) load=$(cut -d' ' -f1 /proc/loadavg)" | tee -a $log }
step fmt
cargo fmt --check > $S/p3-fmt.log 2>&1; echo "fmt EXIT=$?" | tee -a $log
step clippy
cargo clippy --workspace --all-targets -- -D warnings > $S/p3-clippy.log 2>&1; echo "clippy EXIT=$?" | tee -a $log
step llvm-measure
$S/p3-todod-llvm.sh 4 > $S/p3-todod-llvm.out 2>&1; echo "llvm-measure EXIT=$?" | tee -a $log
step failover30
pass=0; fail=0
for i in $(seq 1 30); do
  if cargo test -p ignite --test main failover::a_failing_session_does_not_end_the_service > $S/p3-failover-$i.log 2>&1; then
    pass=$((pass+1))
  else
    fail=$((fail+1)); echo "failover run $i FAILED" | tee -a $log
  fi
done
echo "failover30 pass=$pass fail=$fail" | tee -a $log
step done
echo EXIT=0 | tee -a $log
