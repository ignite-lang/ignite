#!/bin/zsh
# usage: pass2.sh <pair>
# One interleaved pass over the before tree and the fresh after2 snapshot, at whatever load
# (docs/slice.md 15.1; the 1-minute load is recorded per run). Units: warm gate (its gate-test
# step is the warm cargo test step); todod on both targets and profiles; ignite check and
# ignite plan of the design file, five repetitions.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p4-scripts/env.sh
pair=$1
log=$LOGS/pass2-$pair.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) pair=$pair load=$(cat /proc/loadavg) free=$(free_gb)G" | tee -a "$log.summary"
DESIGN=examples/terminal/terminal.design.ignite
TODO=examples/todo/todo.ignite

echo "=== WARM GATE" >>"$log"
for tree in before after2; do
  use_tree $tree
  touch crates/ignite-check/src/lib.rs
  gate "$log" $tree $pair warm
done

echo "=== TODOD" >>"$log"
for profile in dev release; do
  rel=(); [[ $profile == release ]] && rel=(--release)
  for tree in before after2; do
    use_tree $tree
    rt=$RT_DEV; [[ $profile == release ]] && rt=$RT_REL
    out=$BUILDS/$tree-rust-$profile-r$pair
    fresh $out
    SCCACHE_RECACHE=1 timed "$log" $tree "todod-rust-$profile" $pair $IGNITE build $TODO $rel --out $out --target-dir $out-target
    out=$BUILDS/$tree-llvm-$profile-r$pair
    fresh $out
    timed "$log" $tree "todod-llvm-$profile" $pair $IGNITE build $TODO $rel --target llvm --runtime $rt --out $out
  done
done

echo "=== CHECK AND PLAN" >>"$log"
for rep in 1 2 3 4 5; do
  for tree in before after2; do
    use_tree $tree
    timed "$log" $tree check-design $pair-$rep $IGNITE check $DESIGN
  done
  for tree in before after2; do
    use_tree $tree
    timed "$log" $tree plan-design $pair-$rep $IGNITE plan $DESIGN --out $BUILDS/plan2-$tree-$pair-$rep.json
  done
done
finish "$log"
exit $FAILED
