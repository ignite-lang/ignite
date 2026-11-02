#!/bin/zsh
# usage: check-1.sh <log> : fmt check, clippy, then the targets fixture on both targets through the test
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/b11-scripts/env.sh
log=$1
: >$log
cd $REPO || exit 97
cargo fmt --all -- --check >>$log 2>&1; echo "FMT_EXIT=$?" >>$log
cargo clippy --workspace --all-targets -- -D warnings >>$log 2>&1; echo "CLIPPY_EXIT=$?" >>$log
cargo test -p ignite --test main ignite_test::target_bugs >>$log 2>&1; echo "TARGETS_EXIT=$?" >>$log
echo "EXIT=0" >>$log
