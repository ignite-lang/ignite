#!/bin/zsh
# usage: tests-1.sh <log> : bless the new rustgen golden, then time the ignite_test and eval files
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/b11-scripts/env.sh
log=$1
: >$log
cd $REPO || exit 97
IGNITE_BLESS=1 cargo test -p ignite-rustgen --test main golden::target_bugs_of_slice_11b >>$log 2>&1; echo "BLESS_EXIT=$?" >>$log
cargo test -p ignite-rustgen --test main golden:: >>$log 2>&1; echo "GOLDEN_EXIT=$?" >>$log
cargo test -p ignite --test main --no-run >>$log 2>&1; echo "NORUN_EXIT=$?" >>$log
for i in 1 2; do
  t0=$(date +%s.%N)
  cargo test -p ignite --test main ignite_test:: >>$log 2>&1; rc=$?
  t1=$(date +%s.%N)
  echo "IGNITE_TEST run=$i secs=$(echo "$t1 - $t0" | bc) exit=$rc load=$(cut -d' ' -f1 /proc/loadavg)" >>$log
done
t0=$(date +%s.%N)
cargo test -p ignite --test main eval:: >>$log 2>&1; rc=$?
t1=$(date +%s.%N)
echo "EVAL run=1 secs=$(echo "$t1 - $t0" | bc) exit=$rc" >>$log
echo "EXIT=0" >>$log
