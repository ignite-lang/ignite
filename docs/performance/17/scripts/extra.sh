#!/usr/bin/env bash
# The 21.14 units the harness has no mode for, with each side's compiler and runner archives
# that the harness's smoke and programs modes built. One STEP line per run: unit, side, run,
# seconds, 1-minute load at start, exit. Every program or test build has a fresh --out and
# --target-dir, is cleaned after, and runs with SCCACHE_RECACHE=1 when it is a cold unit.
set -u
P=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/s17u6/perf17
T=/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-s17-u6
X=$P/extra
mkdir -p "$X"
SRC_before=$(ls -d "$P"/sources/baseline-*/)
SRC_after=$(ls -d "$P"/sources/candidate-*/)
BLD_before=$(ls -d "$P"/builds/baseline-*/)
BLD_after=$(ls -d "$P"/builds/candidate-*/)
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
clean() {
  local d=$1
  [ -d "$d/cargo" ] && { (cd "$T" && cargo clean --target-dir "$d/cargo" > /dev/null 2>&1); echo "CLEAN $d exit=$?"; }
  rm -rf "$d"
}
rm -rf "$X/std-pre"
mkdir -p "$X/std-pre"
git -C "$T" archive ac043ae8 std | tar -x -C "$X/std-pre"
for i in $(seq 1 15); do
  for side in before after; do
    S=SRC_$side; B=BLD_$side
    timed check-todo "$side" "$i" "${!S}" "${!B}/debug/ignite" check examples/todo/todo.ignite
  done
  for std in pre hooks; do
    root=$SRC_after/std; [ "$std" = pre ] && root=$X/std-pre/std
    timed check-todo-std "$std" "$i" "$SRC_after" "$BLD_after/debug/ignite" check examples/todo/todo.ignite --std "$root"
  done
done
for i in 1 2 3; do
  for profile in dev release; do
    for target in rust llvm; do
      for side in before after; do
        S=SRC_$side; B=BLD_$side
        d=$X/terminald-$side-$target-$profile-$i
        [ -e "$d" ] && { echo "STALE $d"; exit 1; }
        args=(build examples/terminal/terminal.ignite --out "$d/out" --target "$target")
        [ "$profile" = release ] && args+=(--release)
        if [ "$target" = rust ]; then
          args+=(--target-dir "$d/cargo")
        else
          dir=debug; [ "$profile" = release ] && dir=release
          args+=(--runtime "${!B}/ignite-test/rt/$dir/libignite_rt.a")
        fi
        timed "terminald-$target-$profile" "$side" "$i" "${!S}" env SCCACHE_RECACHE=1 "${!B}/debug/ignite" "${args[@]}"
        clean "$d"
      done
      d=$X/studio-$target-$profile-$i
      [ -e "$d" ] && { echo "STALE $d"; exit 1; }
      args=(build crates/ignite/tests/fixtures/slice17/studio.ignite --out "$d/out" --target "$target")
      [ "$profile" = release ] && args+=(--release)
      if [ "$target" = rust ]; then
        args+=(--target-dir "$d/cargo")
      else
        dir=debug; [ "$profile" = release ] && dir=release
        args+=(--runtime "$BLD_after/ignite-test/rt/$dir/libignite_rt.a")
      fi
      timed "studio-$target-$profile" after "$i" "$SRC_after" env SCCACHE_RECACHE=1 "$BLD_after/debug/ignite" "${args[@]}"
      clean "$d"
    done
  done
done
for file in fs process; do
  for target in rust llvm; do
    for i in 1 2 3; do
      d=$X/test-$file-$target-$i
      [ -e "$d" ] && { echo "STALE $d"; exit 1; }
      mkdir -p "$d"
      flags=(--target "$target" --out "$d/out" --target-dir "$d/cargo" --runtime "$BLD_after/ignite-test/rt/debug/libignite_rt.a")
      timed "test-$file-$target-cold" after "$i" "$SRC_after" env SCCACHE_RECACHE=1 "$BLD_after/debug/ignite" test "std/$file.ignite" "${flags[@]}"
      echo "RESULT test-$file-$target-cold $i $(grep -o 'test result: .*' "$X/test-$file-$target-cold-after-$i.out" | head -1)"
      timed "test-$file-$target-warm" after "$i" "$SRC_after" "$BLD_after/debug/ignite" test "std/$file.ignite" "${flags[@]}"
      echo "RESULT test-$file-$target-warm $i $(grep -o 'test result: .*' "$X/test-$file-$target-warm-after-$i.out" | head -1)"
      clean "$d"
    done
  done
done
du -sh "$X"
