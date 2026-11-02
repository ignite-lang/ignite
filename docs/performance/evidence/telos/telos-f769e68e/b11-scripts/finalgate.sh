#!/bin/zsh
# usage: finalgate.sh <log> : the brief's gate on the working tree, each step timed
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/b11-scripts/pairs-env.sh
use_tree after
log=$1
: >"$log"; : >"$log.summary"
echo "START $(date -Is) load=$(cat /proc/loadavg)" | tee -a "$log.summary"
df -h / | tail -1 | tee -a "$log.summary"
timed "$log" work fmt 0 cargo fmt --all -- --check
timed "$log" work clippy 0 cargo clippy --workspace --all-targets -- -D warnings
timed "$log" work test-workspace 0 cargo test --workspace --no-fail-fast
grep -h '^test result:' "$log" | awk '{p+=$4; f+=$6; i+=$8} END {print "COUNTS cargo test --workspace: passed=" p " failed=" f " ignored=" i}' | tee -a "$log.summary"
timed "$log" work test-ignite 0 task test-ignite
grep -h '^running [0-9]* tests' "$log" | tail -2 | tr '\n' ' ' | sed 's/^/INLINE /' | tee -a "$log.summary"; echo | tee -a "$log.summary"
timed "$log" work task-gate 0 task gate
timed "$log" work compile 0 task compile
timed "$log" work compile-llvm 0 task compile-llvm
timed "$log" work compile-todo 0 task compile-todo
timed "$log" work compile-todo-llvm 0 task compile-todo-llvm
echo "EXIT=0" | tee -a "$log.summary"
