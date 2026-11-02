#!/bin/sh
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-i CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
cd /home/timo/projects/telos
cargo test -p telos --no-fail-fast > $S/t3-fulltest.full 2>&1
echo "rc=$?"
grep -E "Running|^test result|FAILED|panicked" $S/t3-fulltest.full
echo EXIT=0
