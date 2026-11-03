#!/bin/zsh
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
cd /home/timo/projects/telos || exit 2
ulimit -c 0
export CARGO_TARGET_DIR=$S/target-ign CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0
green=0
red=0
for i in $(seq 1 30); do
  if cargo test -q -p ignite-rt > $S/logs/rt30-run-$i.log 2>&1; then
    green=$((green + 1))
    echo "run $i: ok"
  else
    red=$((red + 1))
    echo "run $i: FAILED"
    grep -E 'FAILED|panicked' $S/logs/rt30-run-$i.log | head -5
  fi
done
echo "GREEN=$green RED=$red"
echo "EXIT=$red"
