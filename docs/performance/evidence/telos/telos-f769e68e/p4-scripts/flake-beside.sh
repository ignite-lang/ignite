#!/bin/zsh
# usage: flake-beside.sh <n> : run failover::a_failing_session_does_not_end_the_service n times
# while a loop of cargo test --workspace --no-fail-fast runs in the same target dir (15.0 item 3).
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
LOGS=$S/p4-logs
export CARGO_TARGET_DIR=$S/target-p4
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
ulimit -c 0
cd /home/timo/projects/telos || exit 97
n=$1
log=$LOGS/flake-beside.log
: >"$log"
rm -f $LOGS/flake-beside.stop
echo "START $(date -Is) n=$n load=$(cat /proc/loadavg)" >>"$log"
cargo test --workspace --no-run >>"$log" 2>&1
( while [[ ! -f $LOGS/flake-beside.stop ]]; do
    echo "WS-START $(date -Is)" >>$LOGS/flake-beside-workspace.log
    cargo test --workspace --no-fail-fast >>$LOGS/flake-beside-workspace.log 2>&1
    echo "WS-END $(date -Is) exit=$?" >>$LOGS/flake-beside-workspace.log
  done ) &
bgpid=$!
echo "workspace loop pid=$bgpid" >>"$log"
sleep 3
pass=0; fail=0
for i in $(seq 1 $n); do
  cargo test -p ignite --test main failover::a_failing_session_does_not_end_the_service >>"$log" 2>&1
  if [[ $? -eq 0 ]]; then pass=$((pass+1)); else fail=$((fail+1)); echo "FAILED run $i" >>"$log"; fi
done
echo "RESULT beside pass=$pass fail=$fail load=$(cat /proc/loadavg)" >>"$log"
touch $LOGS/flake-beside.stop
wait $bgpid
echo "workspace loop ended $(date -Is)" >>"$log"
grep -c 'WS-END' $LOGS/flake-beside-workspace.log >>"$log"
echo "EXIT=0" >>"$log"
