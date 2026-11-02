#!/bin/zsh
# usage: pass.sh <pair>
# One interleaved pass: for every timed unit, one run on the before tree then one on the after
# tree. Units: warm gate; per test file of crates/ignite/tests after the warm touch; cargo test
# --workspace --no-run warm and after the touch; todod on both targets and profiles (totals);
# ignite check of the design file; ignite plan of the design file (after only).
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/s12-p4-scripts/env.sh
pair=$1
log=$LOGS/pass-$pair.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) pair=$pair load=$(cat /proc/loadavg) free=$(free_gb)G" | tee -a "$log.summary"
DESIGN=examples/terminal/terminal.design.ignite
TODO=examples/todo/todo.ignite

echo "=== WARM GATE" >>"$log"
for tree in before after; do
  use_tree $tree
  touch crates/ignite-check/src/lib.rs
  gate "$log" $tree $pair warm
done

echo "=== PER FILE" >>"$log"
for tree in before after; do
  use_tree $tree
  touch crates/ignite-check/src/lib.rs
  timed "$log" $tree perfile-no-run $pair cargo test -p ignite --test main --no-run
done
files=$( (ls $S/s12-before/crates/ignite/tests/*.rs; ls $S/s12-p4-after/crates/ignite/tests/*.rs) | xargs -n1 basename | sed 's/\.rs$//' | grep -v '^main$' | sort -u)
for f in ${(f)files}; do
  for tree in before after; do
    use_tree $tree
    [[ -f crates/ignite/tests/$f.rs ]] || continue
    if [[ $f == llvm ]]; then
      timed "$log" $tree "file-$f" $pair cargo test -p ignite --test main "llvm::" -- --skip terminal:: --skip todo:: --skip slice10_llvm::
    else
      timed "$log" $tree "file-$f" $pair cargo test -p ignite --test main "$f::"
    fi
  done
done

echo "=== NO-RUN" >>"$log"
for tree in before after; do
  use_tree $tree
  timed "$log" $tree no-run-warm $pair cargo test --workspace --no-run
done
for tree in before after; do
  use_tree $tree
  touch crates/ignite-check/src/lib.rs
  timed "$log" $tree no-run-touch $pair cargo test --workspace --no-run
done

echo "=== TODOD" >>"$log"
for profile in dev release; do
  rel=(); [[ $profile == release ]] && rel=(--release)
  for tree in before after; do
    use_tree $tree
    rt=$RT_DEV; [[ $profile == release ]] && rt=$RT_REL
    out=$BUILDS/$tree-rust-$profile-$pair
    fresh $out
    SCCACHE_RECACHE=1 timed "$log" $tree "todod-rust-$profile" $pair $IGNITE build $TODO $rel --out $out --target-dir $out-target
    out=$BUILDS/$tree-llvm-$profile-$pair
    fresh $out
    timed "$log" $tree "todod-llvm-$profile" $pair $IGNITE build $TODO $rel --target llvm --runtime $rt --out $out
  done
done

echo "=== CHECK AND PLAN" >>"$log"
for rep in 1 2 3 4 5; do
  for tree in before after; do
    use_tree $tree
    timed "$log" $tree check-design $pair-$rep $IGNITE check $DESIGN
  done
  for tree in before after; do
    use_tree $tree
    timed "$log" $tree plan-design $pair-$rep $IGNITE plan $DESIGN --out $BUILDS/plan-$tree-$pair-$rep.json
  done
done
finish "$log"
exit $FAILED
