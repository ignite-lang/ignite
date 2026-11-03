#!/bin/zsh
# usage: pass.sh <pair>
# One interleaved pass: for every timed unit, one run on the before tree then one on the after
# tree. Units: warm gate; per test file of crates/ignite/tests after the warm touch; cargo test
# --workspace --no-run warm and after the touch; todod on both targets and profiles, total and per
# compile step; ignite check of the design file; ignite plan (after only).
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p4-scripts/env.sh
pair=$1
log=$LOGS/pass-$pair.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) pair=$pair load=$(cat /proc/loadavg)" | tee -a "$log.summary"
DESIGN=examples/terminal/terminal.design.ignite
TODO=examples/todo/todo.ignite
triple=$(clang -print-target-triple)

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
files=$( (ls $S/s11-p4-before/crates/ignite/tests/*.rs; ls $S/s11-p4-after/crates/ignite/tests/*.rs) | xargs -n1 basename | sed 's/\.rs$//' | grep -v '^main$' | sort -u)
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
  rel=(); opt=-O0; [[ $profile == release ]] && { rel=(--release); opt=-O2; }
  for tree in before after; do
    use_tree $tree
    rt=$RT_DEV; [[ $profile == release ]] && rt=$RT_REL
    out=$BUILDS/$tree-rust-$profile-$pair
    fresh $out
    SCCACHE_RECACHE=1 timed "$log" $tree "todod-rust-$profile-total" $pair $IGNITE build $TODO $rel --out $out --target-dir $out-target
    split=$out-split
    fresh $split
    SCCACHE_RECACHE=1 timed "$log" $tree "todod-rust-$profile-io" $pair cargo build -q $rel --manifest-path $out/std/Cargo.toml -p ignite-io --target-dir $split
    SCCACHE_RECACHE=1 timed "$log" $tree "todod-rust-$profile-std" $pair cargo build -q $rel --manifest-path $out/std/Cargo.toml --target-dir $split
    SCCACHE_RECACHE=1 timed "$log" $tree "todod-rust-$profile-program" $pair cargo build -q $rel --manifest-path $out/Cargo.toml --target-dir $split
    out=$BUILDS/$tree-llvm-$profile-$pair
    fresh $out
    timed "$log" $tree "todod-llvm-$profile-total" $pair $IGNITE build $TODO $rel --target llvm --runtime $rt --out $out
    work=$out-steps
    fresh $work
    for ll in $out/llvm/*.ll; do
      name=${ll:t:r}
      echo "LINES tree=$tree profile=$profile file=$name lines=$(wc -l <$ll) pair=$pair" | tee -a "$log.summary"
      timed "$log" $tree "todod-llvm-$profile-clang-$name" $pair clang -c $opt -g -ffunction-sections -fdata-sections -Werror=override-module --target=$triple $ll -o $work/$name.o
    done
    strip=(); [[ $profile == release ]] && strip=(-Wl,--strip-debug)
    timed "$log" $tree "todod-llvm-$profile-link" $pair clang -o $work/todod --target=$triple $work/main_todod.o $work/program.o $work/std.o $rt -Wl,--gc-sections $strip -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
  done
done

echo "=== CHECK AND PLAN" >>"$log"
for rep in 1 2 3 4 5; do
  for tree in before after; do
    use_tree $tree
    timed "$log" $tree check-design $pair-$rep $IGNITE check $DESIGN
  done
  use_tree after
  timed "$log" after plan-design $pair-$rep $IGNITE plan $DESIGN --out $BUILDS/plan-$pair-$rep.json
  timed "$log" after explain-design $pair-$rep $IGNITE explain $DESIGN local
done
echo "EXIT=0" | tee -a "$log.summary"
