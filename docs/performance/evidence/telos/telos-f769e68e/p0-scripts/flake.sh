#!/bin/zsh
# usage: flake.sh <mode: alone|beside> <n>
# Runs failover::a_failing_session_does_not_end_the_service n times; in mode beside, a loop of
# cargo test --workspace --no-fail-fast runs in the same target dir until the series ends.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p0-scripts/env.sh
mode=$1; n=$2
log=$LOGS/flake-$mode.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) mode=$mode n=$n load=$(cat /proc/loadavg)" | tee -a "$log.summary"
bgpid=
if [[ $mode == beside ]]; then
  ( while [[ ! -f $LOGS/flake-beside.stop ]]; do
      echo "WS-START $(date +%s.%N)" >>$LOGS/flake-beside-workspace.log
      cargo test --workspace --no-fail-fast >>$LOGS/flake-beside-workspace.log 2>&1
      echo "WS-END $(date +%s.%N) exit=$?" >>$LOGS/flake-beside-workspace.log
    done ) &
  bgpid=$!
  echo "workspace loop pid=$bgpid" | tee -a "$log.summary"
  sleep 3
fi
pass=0; fail=0
for i in $(seq 1 $n); do
  echo "=== RUN $i $(date +%s.%N)" >>"$log"
  timed "$log" "run:$i" cargo test -p ignite --test main failover::a_failing_session_does_not_end_the_service
  if [[ $? -eq 0 ]]; then pass=$((pass+1)); else fail=$((fail+1)); echo "FAILED run $i" | tee -a "$log.summary"; fi
done
echo "RESULT mode=$mode pass=$pass fail=$fail" | tee -a "$log.summary"
if [[ -n $bgpid ]]; then
  touch $LOGS/flake-beside.stop
  wait $bgpid
  echo "workspace loop ended $(date -Is)" | tee -a "$log.summary"
fi
echo "EXIT=0" | tee -a "$log.summary"
