#!/bin/zsh
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
cd /home/timo/projects/telos
export CARGO_TARGET_DIR=$S/target-j CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
s=$(date +%s.%N)
timeout 1500 cargo test -p telos-rustgen --test slice9 long_chains_build_and_run_100k -- --ignored 2>&1 | grep -E '^test |test result|panicked|^error' | head -20
echo "test status=${pipestatus[1]} secs=$(( $(date +%s.%N) - s ))"
echo EXIT=0
