#!/bin/zsh
# Each inline test of fixtures/slice9/regions.ignite alone, both targets, warm build dirs.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p0-scripts/env.sh
log=$LOGS/regions.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) load=$(cat /proc/loadavg)" | tee -a "$log.summary"
IGNITE=$CARGO_TARGET_DIR/debug/ignite
TMP=$CARGO_TARGET_DIR/tmp
RT=$(ls -t $CARGO_TARGET_DIR/debug/deps/libignite_rt-*.a | head -1)
F=crates/ignite/tests/fixtures/slice9/regions.ignite
for target in rust llvm; do
  $IGNITE test $F --list --target $target --out $TMP/ignite-slice9/regions --runtime $RT --target-dir $TMP/ignite-slice9/regions/cargo 2>/dev/null | grep '^main/' | sed 's/: test$//' | while read -r name; do
    echo "=== $target $name" >>"$log"
    timed "$log" "$target:${name// /_}" $IGNITE test $F "$name" --exact --target $target --out $TMP/ignite-slice9/regions --runtime $RT --target-dir $TMP/ignite-slice9/regions/cargo
    grep "finished in" "$log" | tail -1 >>"$log.summary"
  done
done
echo "EXIT=0" | tee -a "$log.summary"
