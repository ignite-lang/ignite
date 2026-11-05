#!/usr/bin/env bash
# The 20.12 units that need only a compiler, run after the ignite-perf trees were cleaned (the
# baseline target passed the 12G limit during warm pass 2). Uses copies of each tree's
# debug/ignite and runner archives taken before the clean. Every program build has its own
# fresh --out and --target-dir and is cleaned after. One STEP line per run: unit, side, run,
# seconds, 1-minute load at start, exit.
set -u
P=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/u6/perf16
X=$P/salvage
mkdir -p "$X"
SRC_before=$P/sources/baseline-31b738b96b67
SRC_after=$P/sources/candidate-426449e9c67f
timed() {
  local unit=$1 side=$2 run=$3 dir=$4; shift 4
  local load start end rc
  load=$(cut -d' ' -f1 /proc/loadavg)
  start=$(date +%s.%N)
  (cd "$dir" && "$@") > "$X/$unit-$side-$run.out" 2>&1
  rc=$?
  end=$(date +%s.%N)
  echo "STEP $unit $side $run $(echo "$end - $start" | bc) $load $rc"
}
for i in $(seq 1 15); do
  for side in before after; do
    S=SRC_$side
    timed check-design "$side" "$i" "${!S}" "$P/bin/$side/ignite" check examples/terminal/terminal.design.ignite
    timed plan-design "$side" "$i" "${!S}" "$P/bin/$side/ignite" plan examples/terminal/terminal.design.ignite --out "$X/plan-$side-$i.json"
    timed check-todo "$side" "$i" "${!S}" "$P/bin/$side/ignite" check examples/todo/todo.ignite
  done
done
for i in 1 2 3; do
  for program in todod terminald; do
    file=examples/todo/todo.ignite
    [ "$program" = terminald ] && file=examples/terminal/terminal.ignite
    for profile in dev release; do
      for target in rust llvm; do
        for side in before after; do
          S=SRC_$side
          d=$X/$program-$side-$target-$profile-$i
          [ -e "$d" ] && { echo "STALE $d"; exit 1; }
          args=(build "$file" --out "$d/out")
          [ "$profile" = release ] && args+=(--release)
          if [ "$target" = rust ]; then
            args+=(--target-dir "$d/cargo")
          else
            dir=debug; [ "$profile" = release ] && dir=release
            args+=(--target llvm --runtime "$P/bin/$side/rt/$dir/libignite_rt.a")
          fi
          timed "$program-$target-$profile" "$side" "$i" "${!S}" env SCCACHE_RECACHE=1 "$P/bin/$side/ignite" "${args[@]}"
          [ -d "$d/cargo" ] && { (cd "${!S}" && cargo clean --target-dir "$d/cargo" > /dev/null 2>&1); echo "CLEAN $d exit=$?"; }
          rm -rf "$d"
        done
      done
    done
  done
done
F=crates/ignite/tests/fixtures/slice16/moves_scale.ignite
for target in rust llvm; do
  for i in 1 2 3; do
    d=$X/moves-scale-$target-$i
    [ -e "$d" ] && { echo "STALE $d"; exit 1; }
    mkdir -p "$d"
    flags=(--target "$target" --out "$d/out" --target-dir "$d/target" --runtime "$P/bin/after/rt/debug/libignite_rt.a")
    timed "moves-scale-$target-cold" after "$i" "$SRC_after" env SCCACHE_RECACHE=1 "$P/bin/after/ignite" test "$F" "${flags[@]}"
    timed "moves-scale-$target-warm" after "$i" "$SRC_after" "$P/bin/after/ignite" test "$F" "${flags[@]}"
    for filter in "50000 steps through a call" "200000 steps through a call"; do
      n=${filter%% *}
      timed "moves-scale-$target-$n" after "$i" "$SRC_after" "$P/bin/after/ignite" test "$F" "$filter" "${flags[@]}"
      echo "FINISHED moves-scale-$target-$n after $i $(grep -o 'finished in [0-9.]*s' "$X/moves-scale-$target-$n-after-$i.out")"
    done
    [ -d "$d/target" ] && { (cd "$SRC_after" && cargo clean --target-dir "$d/target" > /dev/null 2>&1); echo "CLEAN $d exit=$?"; }
    rm -rf "$d"
  done
done
du -sh "$P"
