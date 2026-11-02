#!/bin/zsh
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
export CARGO_TARGET_DIR=$S/target-12b-r2
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
ulimit -c 0
cd /home/timo/projects/telos || exit 97
LOG=$S/r2-gate1.log
: >$LOG
echo "START $(date -Is) load=$(cat /proc/loadavg)" >>$LOG
failed=0
cargo fmt --all -- --check >>$LOG 2>&1; rc=$?; echo "STEP fmt rc=$rc $(date -Is)" >>$LOG; failed=$((failed + rc))
cargo clippy -p ignite-rustgen --all-targets -- -D warnings >>$LOG 2>&1; rc=$?; echo "STEP clippy-rustgen rc=$rc $(date -Is)" >>$LOG; failed=$((failed + rc))
cargo test -p ignite-rustgen >>$LOG 2>&1; rc=$?; echo "STEP test-rustgen rc=$rc $(date -Is)" >>$LOG; failed=$((failed + rc))
echo "EXIT=$failed $(date -Is)" >>$LOG
exit $failed
