#!/bin/zsh
# usage: pairs-all.sh : warm-up of every tree (untimed), the release runtime of the after tree, then
# three interleaved passes and the pair summary; writes EXIT= into pairs.done at the end.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/b11-scripts/pairs-env.sh
: >$LOGS/pairs.progress
rm -f $LOGS/pairs.done
echo "START $(date -Is) load=$(cat /proc/loadavg)" >>$LOGS/pairs.progress
df -h / | tail -1 >>$LOGS/pairs.progress
use_tree after
cargo build -q --release -p ignite-rt --features runner --target-dir $CARGO_TARGET_DIR/ignite-test/rt >>$LOGS/pairs.progress 2>&1
echo "RT_RELEASE exit=$?" >>$LOGS/pairs.progress
for tree in before s11 after; do
  use_tree $tree
  cargo test --workspace --no-fail-fast >$LOGS/pairs-warmup-$tree.log 2>&1
  echo "WARMUP tree=$tree exit=$? $(grep -h '^test result:' $LOGS/pairs-warmup-$tree.log | awk '{p+=$4; f+=$6; i+=$8} END {print "passed=" p " failed=" f " ignored=" i}')" >>$LOGS/pairs.progress
done
for pair in 1 2 3; do
  zsh $S/b11-scripts/pairs-pass.sh $pair >>$LOGS/pairs.progress 2>&1
  echo "PASS $pair DONE $(date -Is) load=$(cat /proc/loadavg)" >>$LOGS/pairs.progress
done
python3 -I $S/b11-scripts/pairs3.py $LOGS/pairs-pass-1.log.summary $LOGS/pairs-pass-2.log.summary $LOGS/pairs-pass-3.log.summary >$LOGS/pairs.txt 2>&1
echo "EXIT=0" >>$LOGS/pairs.done
