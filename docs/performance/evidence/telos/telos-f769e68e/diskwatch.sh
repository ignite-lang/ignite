#!/bin/zsh
SCRATCH=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
OLD=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
LOG=$SCRATCH/diskwatch.log
LIMIT_KB=$((20 * 1024 * 1024))
while true; do
  FREE_KB=$(df -k / | awk 'NR==2 {print $4}')
  if [[ $FREE_KB -lt $LIMIT_KB ]]; then
    for pid in $(pgrep -x cargo; pgrep -x rustc); do
      if tr '\0' '\n' < /proc/$pid/environ 2>/dev/null | grep -q -E "CARGO_TARGET_DIR=($SCRATCH|$OLD)"; then
        kill $pid 2>/dev/null && echo "$(date -Is) free=${FREE_KB}K killed $pid" >> $LOG
      fi
    done
  fi
  sleep 20
done
