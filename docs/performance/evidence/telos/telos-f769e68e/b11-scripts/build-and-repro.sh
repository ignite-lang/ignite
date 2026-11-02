#!/bin/zsh
# usage: build-and-repro.sh <log> <stem>... : build ignite and the runtime, then run the repro fixtures
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/b11-scripts/env.sh
log=$1; shift
: >$log
cd $REPO || exit 97
cargo build -p ignite >>$log 2>&1; echo "BUILD_EXIT=$?" >>$log
cargo build -q -p ignite-rt --features runner --target-dir $CARGO_TARGET_DIR/ignite-test/rt >>$log 2>&1; echo "RT_EXIT=$?" >>$log
zsh $S/b11-scripts/repro.sh "$@" >>$log 2>&1
echo "EXIT=0" >>$log
