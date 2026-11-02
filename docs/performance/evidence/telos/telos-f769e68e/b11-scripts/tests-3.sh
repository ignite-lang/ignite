#!/bin/zsh
# usage: tests-3.sh <log> : the ignite_test file three times, timed, then the full ignite e2e binary once
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/b11-scripts/env.sh
log=$1
: >$log
cd $REPO || exit 97
$IGNITE check crates/ignite/tests/fixtures/tests/targets.ignite >>$log 2>&1; echo "CHECK_EXIT=$?" >>$log
for i in 1 2 3; do
  t0=$(date +%s.%N)
  cargo test -p ignite --test main ignite_test:: >>$log 2>&1; rc=$?
  t1=$(date +%s.%N)
  echo "IGNITE_TEST run=$i secs=$(echo "$t1 - $t0" | bc) exit=$rc load=$(cut -d' ' -f1 /proc/loadavg)" >>$log
done
t0=$(date +%s.%N)
cargo test -p ignite --test main >>$log 2>&1; rc=$?
t1=$(date +%s.%N)
echo "E2E run=1 secs=$(echo "$t1 - $t0" | bc) exit=$rc load=$(cut -d' ' -f1 /proc/loadavg)" >>$log
echo "EXIT=0" >>$log
