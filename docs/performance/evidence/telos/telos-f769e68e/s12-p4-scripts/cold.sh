#!/bin/zsh
# usage: cold.sh before|after...
# Cold gate of each named tree in a fresh target dir with SCCACHE_RECACHE=1; then per tree:
# test-executable count and size, test counts, and the untimed runtime builds pass.sh needs.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p4-scripts/env.sh
for tree in "$@"; do
  use_tree $tree
  log=$LOGS/cold-$tree.log
  : >"$log"; : >"$log.summary"
  echo "START $(date -Is) tree=$tree load=$(cat /proc/loadavg) free=$(free_gb)G" | tee -a "$log.summary"
  fresh $CARGO_TARGET_DIR
  echo "=== COLD $tree tree=$TREE target=$CARGO_TARGET_DIR" >>"$log"
  export SCCACHE_RECACHE=1
  gate "$log" $tree 0 cold
  unset SCCACHE_RECACHE
  echo "=== TEST COUNTS $tree" >>"$log"
  grep -h '^test result:' "$log" | awk -v t=$tree '{p+=$4; f+=$6; i+=$8} END {print "COUNTS tree=" t " passed=" p " failed=" f " ignored=" i}' | tee -a "$log.summary"
  grep -h '^running [0-9]* tests' "$log" | tail -2 | tr '\n' ' ' | sed "s/^/INLINE tree=$tree /" | tee -a "$log.summary"; echo | tee -a "$log.summary"
  echo "=== EXECUTABLES $tree" >>"$log"
  cargo test --workspace --no-run 2>"$BUILDS/no-run-$tree.txt" >/dev/null
  grep -o '(.*)' "$BUILDS/no-run-$tree.txt" | tr -d '()' | sort -u >"$BUILDS/execs-$tree.txt"
  cat "$BUILDS/no-run-$tree.txt" >>"$log"
  printf 'EXECUTABLES tree=%s count=%s bytes=%s\n' $tree "$(wc -l <"$BUILDS/execs-$tree.txt")" "$(cat "$BUILDS/execs-$tree.txt" | xargs -r stat -c %s | awk '{s+=$1} END {print s+0}')" | tee -a "$log.summary"
  echo "=== PREP $tree" >>"$log"
  timed "$log" $tree prep-rt-release 0 cargo build -q --release -p ignite-rt --features runner --target-dir $CARGO_TARGET_DIR/ignite-test/rt
  finish "$log"
done
echo "EXIT=$FAILED failed_steps=[${FAILED_STEPS# }]" >> $LOGS/cold.done
exit $FAILED
