#!/bin/sh
cd /home/timo/projects/telos || exit 9
ulimit -c 0
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-s12-p1
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
LOGS=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p1-logs
cargo test -p ignite-syntax -p ignite-check -p ignite-types -p ignite-eval --no-fail-fast > "$LOGS/test-front2.log" 2>&1
echo "EXIT=$?" >> "$LOGS/test-front2.log"
cargo test -p ignite --no-fail-fast > "$LOGS/test-ignite1.log" 2>&1
echo "EXIT=$?" >> "$LOGS/test-ignite1.log"
echo "EXIT=0"
