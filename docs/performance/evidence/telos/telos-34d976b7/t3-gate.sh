#!/bin/sh
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-i CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
cd /home/timo/projects/telos
echo "== fmt"
rustfmt --edition 2024 --check crates/telos/tests/todo.rs; echo "fmt rc=$?"
echo "== clippy"
cargo clippy -p telos --test todo -- -D warnings 2>&1 | grep -E "^(warning|error)|-->|Finished" | head -40
echo "== test"
cargo test -p telos 2>&1 | grep -E "^test result|FAILED|panicked|failed|^error" | head -40
echo EXIT=0
