#!/bin/zsh
# usage: gate.sh <mode: cold|warm> <run-id>
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p0-scripts/env.sh
mode=$1; run=$2
log=$LOGS/gate-$mode-$run.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) mode=$mode run=$run load=$(cat /proc/loadavg)" | tee -a "$log.summary"
if [[ $mode == warm ]]; then
  touch crates/ignite-check/src/lib.rs
fi
T0=$(date +%s.%N)
timed "$log" fmt cargo fmt --all -- --check
timed "$log" clippy cargo clippy --workspace --all-targets -- -D warnings
timed "$log" test cargo test --workspace --no-fail-fast
timed "$log" test-ignite task test-ignite
T1=$(date +%s.%N)
printf 'TOTAL secs=%.2f load_end=%s\n' "$(echo "$T1 - $T0" | bc)" "$(cat /proc/loadavg)" | tee -a "$log.summary"
echo "EXIT=0" | tee -a "$log.summary"
