#!/bin/zsh
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
NAME=${2:-chains}
cd /home/timo/projects/telos
export CARGO_TARGET_DIR=$S/target-j CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
s=$(date +%s.%N)
timeout 1200 cargo test -p telos-rustgen --test slice9 $1 -- --exact $3 2>&1 | grep -E '^test |test result|panicked|^error' | head -20
echo "test status=${pipestatus[1]} secs=$(( $(date +%s.%N) - s ))"
C=$S/target-j/telos-rustgen-slice9
touch $C/$NAME/src/lib.rs
s=$(date +%s.%N)
RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= timeout 1200 cargo build --manifest-path $C/$NAME/Cargo.toml --target-dir $C/$NAME-target 2>&1 | tail -1
echo "rebuild status=${pipestatus[1]} secs=$(( $(date +%s.%N) - s ))"
wc -c -l $C/$NAME/src/lib.rs
echo EXIT=0
