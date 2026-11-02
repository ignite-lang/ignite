#!/bin/sh
cd /home/timo/projects/telos || exit 9
ulimit -c 0
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
LOGS=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p1-logs
rustfmt --edition 2024 crates/ignite-eval/src/validate.rs crates/ignite-eval/src/resolve.rs > "$LOGS/rustfmt3.log" 2>&1
echo "EXIT=$?" >> "$LOGS/rustfmt3.log"
CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-s12-p1 cargo test -p ignite-syntax -p ignite-check -p ignite-types --no-fail-fast > "$LOGS/test-front4.log" 2>&1
echo "EXIT=$?" >> "$LOGS/test-front4.log"
cd /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-before || exit 9
CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-s12-p1-before cargo build -p ignite > "$LOGS/build-before1.log" 2>&1
echo "EXIT=$?" >> "$LOGS/build-before1.log"
echo "EXIT=0"
