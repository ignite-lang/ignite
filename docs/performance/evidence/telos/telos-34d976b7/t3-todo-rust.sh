#!/bin/sh
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-i CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
cd /home/timo/projects/telos
rustfmt --edition 2024 crates/telos/tests/todo.rs
cargo test -p telos --test todo 2>&1 | grep -vE "^\s+Compiling" | tail -80
echo EXIT=0
