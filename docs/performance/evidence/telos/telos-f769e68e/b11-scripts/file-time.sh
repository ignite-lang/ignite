#!/bin/zsh
# usage: file-time.sh LOG : time `cargo test -p ignite --test main ignite_test::` three times
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/b11-scripts/env.sh
cd $REPO
for i in 1 2 3; do
  uptime
  t0=$(date +%s.%N)
  cargo test -p ignite --test main ignite_test:: 2>&1 | grep -E '^test result|^test .*(ok|FAILED)$|Finished|error' | tail -n 16
  rc=${pipestatus[1]}
  t1=$(date +%s.%N)
  printf 'STEP file-ignite_test-%d %.2f s exit=%s\n' $i $(echo "$t1 - $t0" | bc) $rc
done
echo "EXIT=0"
