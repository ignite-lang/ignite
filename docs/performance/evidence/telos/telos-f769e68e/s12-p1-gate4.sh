#!/bin/sh
cd /home/timo/projects/telos || exit 9
ulimit -c 0
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-s12-p1
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
LOGS=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p1-logs
cargo build -p ignite > "$LOGS/build-ignite4.log" 2>&1
rc=$?
echo "EXIT=$rc" >> "$LOGS/build-ignite4.log"
if [ "$rc" -ne 0 ]; then
  echo "EXIT=build-failed"
  exit 0
fi
cargo test -p ignite --test main plan::fixtures_match_expected > "$LOGS/test-fixtures4.log" 2>&1
echo "EXIT=$?" >> "$LOGS/test-fixtures4.log"
cargo test -p ignite --test main cli::check_design_file_is_silent >> "$LOGS/test-fixtures4.log" 2>&1
echo "EXIT=$?" >> "$LOGS/test-fixtures4.log"
cargo test -p ignite-eval --no-fail-fast > "$LOGS/test-eval4.log" 2>&1
echo "EXIT=$?" >> "$LOGS/test-eval4.log"
sh /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p1-check-all.sh > "$LOGS/check-all4.log" 2>&1
sh /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p1-timing.sh > "$LOGS/timing4.log" 2>&1
echo "EXIT=0"
