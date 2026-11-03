#!/bin/zsh
# usage: ab-ignite-test.sh LOG : A/B of ignite_test.rs (keep = shared out dir, new = per-invocation dirs), 3 pairs, harness "finished in" time
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/b11-scripts/env.sh
cd $REPO
T=crates/ignite/tests/ignite_test.rs
KEEP=$BUILDS/ignite_test.rs.keep
NEW=$BUILDS/ignite_test.rs.new
cp $T $NEW
run() {
  local label=$1
  local load=$(cut -d' ' -f1 /proc/loadavg)
  local line=$(cargo test -p ignite --test main ignite_test:: 2>&1 | grep -E '^test result')
  printf 'STEP %s load=%s %s\n' $label $load "$line"
}
for p in 1 2 3; do
  cp $KEEP $T; run keep-$p
  cp $NEW $T; run new-$p
done
cp $NEW $T
cmp -s $NEW $T && echo "restored new version"
echo "EXIT=0"
