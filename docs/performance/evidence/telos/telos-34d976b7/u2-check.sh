#!/bin/sh
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-j CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_DEBUG=0
cd /home/timo/projects/telos || exit 1
ulimit -c 0
cargo fmt -p telos-io -p telos-rt -p telos-rustgen -p telos-llvm -p telos
echo "fmt=$?"
cargo fmt --all -- --check 2>&1 | grep "^Diff in" | head
echo "fmtcheck=$?"
cargo clippy -q --workspace --all-targets -- -D warnings 2>&1 | tail -40
echo "clippy=$?"
cargo clippy -q -p telos-io -p telos-rt --all-targets --features runner -- -D warnings 2>&1 | tail -40
echo "clippy-runner=$?"
cargo test -q -p telos-io -p telos-rt -p telos-rustgen -p telos-llvm 2>&1 | grep -E "^test result|FAILED|panicked|error" | head -40
echo "unit=$?"
cargo test -q -p telos-io --features runner 2>&1 | grep -E "^test result|FAILED|panicked|error" | head -20
cargo test -q -p telos --test telos_test 2>&1 | tail -15
echo EXIT=0
