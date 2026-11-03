#!/bin/zsh
# usage: perfile.sh <run-id>
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p0-scripts/env.sh
run=$1
log=$LOGS/perfile-$run.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) run=$run load=$(cat /proc/loadavg)" | tee -a "$log.summary"
touch crates/ignite-check/src/lib.rs
timed "$log" no-run cargo test -p ignite --test main --no-run
for f in cli failover generics ignite_test llvm_build llvm_memory siblings slice10_llvm slice9 syntax terminal todo; do
  echo "=== FILE $f" >>"$log"
  timed "$log" "file:$f" cargo test -p ignite --test main "$f::"
done
echo "=== FILE llvm (skipping terminal:: todo:: slice10_llvm::, whose llvm submodules the substring filter matches)" >>"$log"
timed "$log" "file:llvm" cargo test -p ignite --test main "llvm::" -- --skip terminal:: --skip todo:: --skip slice10_llvm::
echo "=== WHOLE" >>"$log"
timed "$log" whole-ignite-main cargo test -p ignite --test main
echo "EXIT=0" | tee -a "$log.summary"
