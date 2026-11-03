#!/bin/zsh
# usage: cold.sh
# One cold gate per tree, each in a scratch target dir emptied with cargo clean first, with
# SCCACHE_RECACHE=1. The after tree runs first in $P/target-after, which is emptied again right
# after (disk budget); then the before tree in $P/target-before, which pass.sh reuses warm.
# Then the untimed release runtimes the LLVM release builds of pass.sh need.
source /home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/s14/perf/scripts/env.sh
log=$LOGS/cold.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) load=$(cat /proc/loadavg) free=$(free_gb)G" | tee -a "$log.summary"
use_tree after-cold
clean $CARGO_TARGET_DIR
export SCCACHE_RECACHE=1
gate "$log" after 0 cold
unset SCCACHE_RECACHE
echo "SIZE target-after $(du -sh $CARGO_TARGET_DIR | cut -f1)" | tee -a "$log.summary"
clean $CARGO_TARGET_DIR
use_tree before
clean $CARGO_TARGET_DIR
export SCCACHE_RECACHE=1
gate "$log" before 0 cold
unset SCCACHE_RECACHE
for tree in before after; do
  use_tree $tree
  timed "$log" $tree prep-rt-release 0 cargo build -q --release -p ignite-rt --features runner --target-dir ${CARGO_TARGET_DIR:-target}/ignite-test/rt
done
finish "$log"
