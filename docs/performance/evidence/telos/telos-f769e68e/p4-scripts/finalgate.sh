#!/bin/zsh
# usage: finalgate.sh : the brief's gate on the working tree, each step timed, plus item 8.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p4-scripts/env.sh
export CARGO_TARGET_DIR=$S/target-p4
cd /home/timo/projects/telos || exit 97
log=$LOGS/finalgate.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) load=$(cat /proc/loadavg)" | tee -a "$log.summary"
timed "$log" work item8-eval 0 cargo test -p ignite --test main eval::
timed "$log" work fmt 0 cargo fmt --all -- --check
timed "$log" work clippy 0 cargo clippy --workspace --all-targets -- -D warnings
timed "$log" work test-workspace 0 cargo test --workspace
timed "$log" work test-ignite 0 task test-ignite
timed "$log" work task-gate 0 task gate
timed "$log" work compile 0 task compile
timed "$log" work compile-llvm 0 task compile-llvm
timed "$log" work compile-todo 0 task compile-todo
timed "$log" work compile-todo-llvm 0 task compile-todo-llvm
grep -h '^test result:' "$log" | awk '{p+=$4; f+=$6; i+=$8} END {print "COUNTS (all steps, double-counted by task gate) passed=" p " failed=" f " ignored=" i}' | tee -a "$log.summary"
echo "EXIT=0" | tee -a "$log.summary"
