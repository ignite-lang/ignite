#!/bin/zsh
# usage: finalgate.sh : the brief's gate on the working tree, each step timed, in the P4 target dir.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p4-scripts/env.sh
export CARGO_TARGET_DIR=$S/target-s12-p4
cd /home/timo/projects/telos || exit 97
log=$LOGS/finalgate.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) load=$(cat /proc/loadavg) free=$(free_gb)G" | tee -a "$log.summary"
timed "$log" work fmt 0 cargo fmt --all -- --check
timed "$log" work clippy 0 cargo clippy --workspace --all-targets -- -D warnings
timed "$log" work test-workspace 0 cargo test --workspace --no-fail-fast
timed "$log" work test-ignite 0 task test-ignite
timed "$log" work task-gate 0 task gate
timed "$log" work compile 0 task compile
timed "$log" work compile-llvm 0 task compile-llvm
timed "$log" work compile-todo 0 task compile-todo
timed "$log" work compile-todo-llvm 0 task compile-todo-llvm
grep -h '^test result:' "$log" | awk '{p+=$4; f+=$6; i+=$8} END {print "COUNTS (all steps, double-counted by task gate) passed=" p " failed=" f " ignored=" i}' | tee -a "$log.summary"
grep -h '^running [0-9]* tests' "$log" | sort | uniq -c | sed 's/^/RUNNING /' | tee -a "$log.summary"
grep -h -E '^test .* FAILED$' "$log" | sort | uniq -c | sed 's/^/FAILED /' | tee -a "$log.summary"
finish "$log"
exit $FAILED
