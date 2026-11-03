#!/bin/zsh
# usage: pass.sh <pair>
# One interleaved warm pass: every unit of 18.18 once on the before tree, then once on the after
# tree. Units: the warm gate after the touch (apply skipped, steps timed separately);
# `task test-ignite` with nothing changed; `--no-run` after the touch; check and plan of the
# design file (5 pairs per pass); `ignite test` of telnet.ignite and http.ignite, cold
# (fresh --out and target, SCCACHE_RECACHE=1) then warm (same dirs); todod and terminald,
# Rust and LLVM, dev and release (fresh --out and target, SCCACHE_RECACHE=1).
source /home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/s14/perf/scripts/env.sh
pair=$1
log=$LOGS/pass-$pair.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) pair=$pair load=$(cat /proc/loadavg) free=$(free_gb)G" | tee -a "$log.summary"

for tree in before after; do
  use_tree $tree
  touch crates/ignite-check/src/lib.rs
  gate "$log" $tree $pair warm
done

for tree in before after; do
  use_tree $tree
  timed "$log" $tree test-ignite-warm $pair task test-ignite
done

for tree in before after; do
  use_tree $tree
  touch crates/ignite-check/src/lib.rs
  timed "$log" $tree no-run-touch $pair cargo test --workspace --no-run
done

for i in 1 2 3 4 5; do
  for tree in before after; do
    use_tree $tree
    timed "$log" $tree check-design $pair.$i $IGNITE check $DESIGN
  done
  for tree in before after; do
    use_tree $tree
    timed "$log" $tree plan-design $pair.$i $IGNITE plan $DESIGN --out $BUILDS/plan-$tree.json
  done
done

for file in telnet http; do
  for tree in before after; do
    use_tree $tree
    out=$BUILDS/$tree-test-$file-$pair
    clean_build $out
    export SCCACHE_RECACHE=1
    timed "$log" $tree ignite-test-$file-cold $pair $IGNITE test std/protocol/$file.ignite --target all --out $out --target-dir $out/cargo --runtime $RT_DEV
    unset SCCACHE_RECACHE
  done
  for tree in before after; do
    use_tree $tree
    out=$BUILDS/$tree-test-$file-$pair
    timed "$log" $tree ignite-test-$file-warm $pair $IGNITE test std/protocol/$file.ignite --target all --out $out --target-dir $out/cargo --runtime $RT_DEV
  done
  for tree in before after; do
    use_tree $tree
    clean_build $BUILDS/$tree-test-$file-$pair
  done
done

for prog in todod terminald; do
  src=$TODO; [[ $prog == terminald ]] && src=$TERMINAL
  for target in rust llvm; do
    for profile in dev release; do
      rel=(); [[ $profile == release ]] && rel=(--release)
      for tree in before after; do
        use_tree $tree
        rt=$RT_DEV; [[ $profile == release ]] && rt=$RT_REL
        out=$BUILDS/$tree-$prog-$target-$profile-$pair
        clean_build $out
        export SCCACHE_RECACHE=1
        if [[ $target == rust ]]; then
          timed "$log" $tree $prog-$target-$profile $pair $IGNITE build $src $rel --out $out/out --target-dir $out/target
        else
          timed "$log" $tree $prog-$target-$profile $pair $IGNITE build $src --target llvm $rel --out $out/out --runtime $rt
        fi
        unset SCCACHE_RECACHE
        clean_build $out
      done
    done
  done
done

finish "$log"
