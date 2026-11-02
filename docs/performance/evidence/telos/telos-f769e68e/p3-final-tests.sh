#!/bin/zsh
# P3: final test pass over the crates P3 touched: ignite-eval, ignite-llvm, ignite.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
cd /home/timo/projects/telos || exit 9
export CARGO_TARGET_DIR=$S/target-p3 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0
ulimit -c 0
log=$S/p3-final-tests.log
: > $log
for crate in ignite-eval ignite-llvm ignite; do
  cargo test -p $crate > $S/p3-final-$crate.log 2>&1; rc=$?
  echo "$crate EXIT=$rc $(grep -h 'test result' $S/p3-final-$crate.log | tr '\n' ';')" | tee -a $log
done
echo EXIT=0 | tee -a $log
