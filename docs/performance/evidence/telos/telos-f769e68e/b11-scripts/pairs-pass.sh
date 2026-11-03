#!/bin/zsh
# usage: pairs-pass.sh <pair> : one interleaved pass over the trees before (60aef38), s11 (slice-11
# close) and after (the working tree): the warm cargo test --workspace step and the ignite_test
# file after the touch, then todod on both targets and profiles (before and after only).
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/b11-scripts/pairs-env.sh
pair=$1
log=$LOGS/pairs-pass-$pair.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) pair=$pair load=$(cat /proc/loadavg)" | tee -a "$log.summary"
TODO=examples/todo/todo.ignite

echo "=== WARM TEST STEP" >>"$log"
for tree in before s11 after; do
  use_tree $tree
  touch crates/ignite-check/src/lib.rs
  timed "$log" $tree warm-test $pair cargo test --workspace --no-fail-fast
done

echo "=== IGNITE_TEST FILE" >>"$log"
for tree in before s11 after; do
  use_tree $tree
  timed "$log" $tree file-ignite_test $pair cargo test -p ignite --test main ignite_test::
done

echo "=== TODOD" >>"$log"
for profile in dev release; do
  rel=(); [[ $profile == release ]] && rel=(--release)
  for tree in before after; do
    use_tree $tree
    rt=$RT_DEV; [[ $profile == release ]] && rt=$RT_REL
    out=$BUILDS/pairs/$tree-rust-$profile-$pair
    fresh $out
    SCCACHE_RECACHE=1 timed "$log" $tree "todod-rust-$profile" $pair $IGNITE build $TODO $rel --out $out --target-dir $out-target
    out=$BUILDS/pairs/$tree-llvm-$profile-$pair
    fresh $out
    timed "$log" $tree "todod-llvm-$profile" $pair $IGNITE build $TODO $rel --target llvm --runtime $rt --out $out
  done
done
echo "EXIT=0" | tee -a "$log.summary"
