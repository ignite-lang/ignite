#!/bin/sh
cd /home/timo/projects/telos || exit 9
ulimit -c 0
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-s12-p1
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
LOGS=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p1-logs
T=$CARGO_TARGET_DIR
cargo fmt --all -- --check > "$LOGS/fmt1.log" 2>&1
echo "EXIT=$?" >> "$LOGS/fmt1.log"
cargo clippy --workspace --all-targets -- -D warnings > "$LOGS/clippy1.log" 2>&1
echo "EXIT=$?" >> "$LOGS/clippy1.log"
cargo build -q -p ignite > "$LOGS/inline1.log" 2>&1
cargo build -q -p ignite-rt --features runner --target-dir "$T/ignite-test/rt" >> "$LOGS/inline1.log" 2>&1
FILES=$(find std examples -name '*.ignite' ! -name '*.design.ignite' | sort | tr '\n' ' ')
"$T/debug/ignite" test $FILES --out "$T/ignite-test" --target-dir "$T/ignite-test/cargo" >> "$LOGS/inline1.log" 2>&1
echo "RUST_EXIT=$?" >> "$LOGS/inline1.log"
"$T/debug/ignite" test $FILES --target llvm --out "$T/ignite-test" --runtime "$T/ignite-test/rt/debug/libignite_rt.a" >> "$LOGS/inline1.log" 2>&1
echo "LLVM_EXIT=$?" >> "$LOGS/inline1.log"
echo "EXIT=0" >> "$LOGS/inline1.log"
echo "EXIT=0"
