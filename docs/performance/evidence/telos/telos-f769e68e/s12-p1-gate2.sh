#!/bin/sh
cd /home/timo/projects/telos || exit 9
ulimit -c 0
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-s12-p1
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
LOGS=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p1-logs
cargo test -p ignite-syntax -p ignite-check -p ignite-types -p ignite-eval --no-fail-fast > "$LOGS/test-front3.log" 2>&1
echo "EXIT=$?" >> "$LOGS/test-front3.log"
cargo build --workspace --all-targets > "$LOGS/build2.log" 2>&1
echo "EXIT=$?" >> "$LOGS/build2.log"
cargo clippy --workspace --all-targets -- -D warnings > "$LOGS/clippy2.log" 2>&1
echo "EXIT=$?" >> "$LOGS/clippy2.log"
cargo fmt --all -- --check > "$LOGS/fmt2.log" 2>&1
echo "EXIT=$?" >> "$LOGS/fmt2.log"
sh /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p1-check-all.sh > "$LOGS/check-all2.log" 2>&1
echo "EXIT=0"
