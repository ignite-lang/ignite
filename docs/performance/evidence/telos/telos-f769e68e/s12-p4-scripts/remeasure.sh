#!/bin/zsh
# usage: remeasure.sh : fresh snapshot of the working tree as after2, its cold gate in a fresh
# target dir, three interleaved passes (pass2.sh) against the before tree, the pair summary.
# The final EXIT= line carries the number of valid gate-test pairs.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p4-scripts/env.sh
P=$LOGS/remeasure.progress
: >$P
echo "START $(date -Is) load=$(cat /proc/loadavg) free=$(free_gb)G" >>$P
if [[ -e $S/s12-p4-after2 ]]; then echo "snapshot exists, refusing" >>$P; echo "EXIT=0 refused" >>$LOGS/remeasure.done; exit 96; fi
rsync -a --exclude /target --exclude /.git /home/timo/projects/telos/ $S/s12-p4-after2/ >>$P 2>&1
echo "SNAPSHOT $(date -Is) files=$(find $S/s12-p4-after2 -type f | wc -l)" >>$P
failed=0
zsh $S/s12-p4-scripts/cold.sh after2 >>$P 2>&1
rc=$?; failed=$((failed + rc))
echo "COLD DONE $(date -Is) failed_steps=$rc load=$(cat /proc/loadavg) free=$(free_gb)G" >>$P
for pair in 1 2 3; do
  zsh $S/s12-p4-scripts/pass2.sh $pair >>$P 2>&1
  rc=$?; failed=$((failed + rc))
  echo "PASS $pair DONE $(date -Is) failed_steps=$rc load=$(cat /proc/loadavg) free=$(free_gb)G" >>$P
done
python3 -I $S/s12-p4-scripts/pairs.py $LOGS/cold-before.log.summary $LOGS/cold-after2.log.summary $LOGS/pass2-1.log.summary $LOGS/pass2-2.log.summary $LOGS/pass2-3.log.summary >$LOGS/pairs2.txt 2>&1
valid=$(awk '/^## gate-test$/{f=1;next} /^## /{f=0} f' $LOGS/pairs2.txt | grep -o 'pairs kept [0-9]*' | awk '{print $3}')
echo "DONE $(date -Is) failed_steps=$failed valid_gate_test_pairs=${valid:-0}" >>$P
echo "EXIT=${valid:-0} failed_steps=$failed" >>$LOGS/remeasure.done
exit $failed
