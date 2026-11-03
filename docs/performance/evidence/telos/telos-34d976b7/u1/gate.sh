#!/usr/bin/env bash
ulimit -c 0
cd /home/timo/projects/telos || exit 1
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
echo "== test"
cargo test -p telos-syntax -p telos-check -p telos-types 2>&1 | grep -E '^test result|FAILED|panicked|^error'
echo "TEST_EXIT=${PIPESTATUS[0]}"
echo "== build"
cargo build --workspace --all-targets 2>&1 | grep -E '^(error|warning)' -A6 | head -60
echo "BUILD_EXIT=${PIPESTATUS[0]}"
echo "== clippy own"
cargo clippy -p telos-syntax -p telos-check -p telos-types --all-targets -- -D warnings 2>&1 | grep -E '^(error|warning)' -A8 | head -80
echo "CLIPPY_OWN_EXIT=${PIPESTATUS[0]}"
echo "== clippy workspace"
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -E '^(error|warning)' -A4 | grep -E '^(error|warning)|-->' | head -40
echo "CLIPPY_WS_EXIT=${PIPESTATUS[0]}"
echo "== fmt"
cargo fmt --all --check 2>&1 | grep '^Diff in' | head -30
echo "FMT_EXIT=${PIPESTATUS[0]}"
echo "== telos check"
cargo build -p telos 2>&1 | tail -1
TELOS=$CARGO_TARGET_DIR/debug/telos
for f in std/*.telos std/protocol/*.telos examples/terminal/terminal.telos examples/todo/todo.telos crates/telos-types/tests/fixtures/valid_tests.telos crates/telos-syntax/tests/fixtures/valid_tests.telos; do
  out=$("$TELOS" check "$f" 2>&1); code=$?
  echo "$code $f ${out:0:300}"
done
echo "EXIT=done"
