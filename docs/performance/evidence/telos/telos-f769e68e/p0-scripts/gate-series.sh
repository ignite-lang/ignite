#!/bin/zsh
# usage: gate-series.sh <first-run> <last-run>
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
for i in $(seq $1 $2); do
  zsh $S/p0-scripts/gate.sh warm $i
done
echo "EXIT=0" >> $S/p0-logs/gate-series-$1-$2.done
