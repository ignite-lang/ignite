#!/usr/bin/env bash
cd /home/timo/projects/telos
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-i CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
O=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/gate-j
cargo fmt --all --check > $O.fmt 2>&1; echo "FMT=$?"
cargo clippy --workspace --all-targets -- -D warnings > $O.clippy 2>&1; echo "CLIPPY=$?"
cargo test -p telos-io -p telos-rt -p telos -p telos-rustgen > $O.test 2>&1; echo "TEST=$?"
echo EXIT=0
