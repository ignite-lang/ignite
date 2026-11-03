#!/bin/zsh
# usage: close.sh : snapshot the working tree as the after tree, cold gate of the after tree
# (the before tree's cold gate ran earlier), three interleaved passes, then the pair summary.
# Writes EXIT= into close.done at the end.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
LOGS=$S/s12-p4-logs/measure
mkdir -p $LOGS
: >$LOGS/close.progress
echo "START $(date -Is) load=$(cat /proc/loadavg)" >>$LOGS/close.progress
if [[ -e $S/s12-p4-after ]]; then echo "snapshot exists, refusing" >>$LOGS/close.progress; echo "EXIT=96" >>$LOGS/close.done; exit 96; fi
rsync -a --exclude /target --exclude /.git /home/timo/projects/telos/ $S/s12-p4-after/ >>$LOGS/close.progress 2>&1
echo "SNAPSHOT $(date -Is) files=$(find $S/s12-p4-after -type f | wc -l)" >>$LOGS/close.progress
df -h / | tail -1 >>$LOGS/close.progress
failed=0
zsh $S/s12-p4-scripts/cold.sh after >>$LOGS/close.progress 2>&1
rc=$?; failed=$((failed + rc))
echo "COLD DONE $(date -Is) failed_steps=$rc load=$(cat /proc/loadavg)" >>$LOGS/close.progress
df -h / | tail -1 >>$LOGS/close.progress
for pair in 1 2 3; do
  zsh $S/s12-p4-scripts/pass.sh $pair >>$LOGS/close.progress 2>&1
  rc=$?; failed=$((failed + rc))
  echo "PASS $pair DONE $(date -Is) failed_steps=$rc load=$(cat /proc/loadavg)" >>$LOGS/close.progress
  df -h / | tail -1 >>$LOGS/close.progress
done
python3 -I $S/s12-p4-scripts/pairs.py $LOGS/cold-before.log.summary $LOGS/cold-after.log.summary $LOGS/pass-1.log.summary $LOGS/pass-2.log.summary $LOGS/pass-3.log.summary >$LOGS/pairs.txt 2>&1
rc=$?; failed=$((failed + rc))
echo "EXIT=$failed" >>$LOGS/close.done
exit $failed
