#!/bin/zsh
# usage: s9b-gate.sh <outprefix>   the steps of task gate, each to its own log, continuing after failures
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
O=$1
cd /home/timo/projects/telos
export CARGO_TARGET_DIR=$S/target-k CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
cargo fmt --all -- --check > $O.fmt 2>&1; echo "FMT=$?" > $O.sum
cargo clippy --workspace --all-targets -- -D warnings > $O.clippy 2>&1; echo "CLIPPY=$?" >> $O.sum
cargo test --workspace --no-fail-fast > $O.test 2>&1; echo "TEST=$?" >> $O.sum
task test-telos > $O.tt 2>&1; echo "TELOS=$?" >> $O.sum
grep -h '^test result' $O.test | awk '{p+=$4; f+=$6; i+=$8} END {printf "cargo_tests passed=%d failed=%d ignored=%d\n", p, f, i}' >> $O.sum
grep -h '^test .*FAILED' $O.test >> $O.sum
grep -h '^test result' $O.tt >> $O.sum
echo EXIT=0 >> $O.sum
