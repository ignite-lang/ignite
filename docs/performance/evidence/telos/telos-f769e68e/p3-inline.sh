#!/bin/zsh
# P3: inline Ignite tests of std/ and examples/ on both targets, as Taskfile test-ignite does,
# with this session's target dir.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
cd /home/timo/projects/telos || exit 9
export CARGO_TARGET_DIR=$S/target-p3 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0
T=$S/target-p3
ulimit -c 0
log=$S/p3-inline.log
: > $log
FILES=($(find std examples -name '*.ignite' ! -name '*.design.ignite' | sort))
echo "FILES ${#FILES}" | tee -a $log
cargo build -q -p ignite >> $log 2>&1; echo "build ignite EXIT=$?" | tee -a $log
cargo build -q -p ignite-rt --features runner --target-dir $T/ignite-test/rt >> $log 2>&1; echo "build rt EXIT=$?" | tee -a $log
t0=$(date +%s.%N)
$T/debug/ignite test $FILES --out $T/ignite-test --target-dir $T/ignite-test/cargo > $S/p3-inline-rust.log 2>&1; rc=$?
t1=$(date +%s.%N)
printf 'rust EXIT=%s secs=%.1f\n' $rc $(echo "$t1 - $t0" | bc) | tee -a $log
t0=$(date +%s.%N)
$T/debug/ignite test $FILES --target llvm --out $T/ignite-test --runtime $T/ignite-test/rt/debug/libignite_rt.a > $S/p3-inline-llvm.log 2>&1; rc=$?
t1=$(date +%s.%N)
printf 'llvm EXIT=%s secs=%.1f\n' $rc $(echo "$t1 - $t0" | bc) | tee -a $log
echo EXIT=0 | tee -a $log
