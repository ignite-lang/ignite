#!/bin/zsh
# usage: chain1.sh : warm-up run of the cut ignite_test.rs on the working tree (its three cargo
# dirs are cold), then three pertest2.sh passes, then the pair summaries. EXIT= at the end.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
LOGS=$S/s12b-r3-logs
P=$LOGS/chain1.progress
: >$P
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
echo "START $(date -Is) load=$(cat /proc/loadavg)" >>$P
cd /home/timo/projects/telos || exit 97
export CARGO_TARGET_DIR=$S/target-12b-r3
t0=$(date +%s.%N)
cargo test -p ignite --test main "ignite_test::" >$LOGS/warmup-work.log 2>&1
rc=$?
echo "WARMUP secs=$(echo "$(date +%s.%N) - $t0" | bc) exit=$rc load=$(cut -d' ' -f1 /proc/loadavg)" >>$P
grep -E '^test |^test result' $LOGS/warmup-work.log >>$P
failed=$rc
for pass in 1 2 3; do
  zsh $S/s12b-r3-scripts/pertest2.sh $pass >>$P 2>&1
  rc=$?; failed=$((failed + rc))
  echo "PASS $pass DONE $(date -Is) failed_steps=$rc load=$(cat /proc/loadavg)" >>$P
done
python3 -I $S/s12b-r3-scripts/pairs.py before work $LOGS/pertest2-{1,2,3}.log.summary >$LOGS/pairs-before-work.txt 2>&1
python3 -I $S/s12b-r3-scripts/pairs.py s12 work $LOGS/pertest2-{1,2,3}.log.summary >$LOGS/pairs-s12-work.txt 2>&1
python3 -I $S/s12b-r3-scripts/pairs.py before s12 $LOGS/pertest2-{1,2,3}.log.summary >$LOGS/pairs-before-s12.txt 2>&1
echo "EXIT=$failed" >>$P
exit $failed
