#!/bin/zsh
# usage: pertest.sh <run-id> <file>... : each file serialised with timestamps per test line
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p0-scripts/env.sh
run=$1; shift
for f in "$@"; do
  log=$LOGS/pertest-$run-$f.log
  : >"$log"
  echo "START $(date -Is) file=$f load=$(cat /proc/loadavg)" >>"$log"
  cargo test -p ignite --test main "$f::" -- --test-threads=1 2>&1 | python3 -I $S/p0-scripts/ts.py >>"$log"
  echo "END load=$(cat /proc/loadavg)" >>"$log"
done
echo "EXIT=0" >> $LOGS/pertest-$run.done
