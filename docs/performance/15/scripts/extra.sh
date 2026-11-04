#!/usr/bin/env bash
# The units 19.17 adds to ignite-perf's: `ignite check` of the todo example (15 interleaved
# pairs), `ignite conformance` of telnet.ignite and http.ignite per target, cold and warm
# (after only, 3 runs each), `ignite rfc import` of RFC 9110 with D4's sections (after only,
# 5 runs). One STEP line per run: unit, side, run, seconds, 1-minute load at start, exit.
set -u
P=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/perf15
X=$P/extra
mkdir -p "$X"
BEFORE_SRC=$(ls -d "$P"/sources/baseline-*)
AFTER_SRC=$(ls -d "$P"/sources/candidate-*)
BEFORE_BIN=$P/builds/baseline-296f84973552/debug/ignite
AFTER_T=$P/builds/candidate-c20f04b6fead
AFTER_BIN=$AFTER_T/debug/ignite
RT=$AFTER_T/ignite-test/rt/debug/libignite_rt.a
echo "TREES before=$BEFORE_SRC after=$AFTER_SRC"
timed() { # unit side run dir cmd...
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
  timed check-todo before "$i" "$BEFORE_SRC" "$BEFORE_BIN" check examples/todo/todo.ignite
  timed check-todo after "$i" "$AFTER_SRC" "$AFTER_BIN" check examples/todo/todo.ignite
done
for name in telnet http; do
  for target in rust llvm; do
    for i in 1 2 3; do
      d=$X/conf-$name-$target-$i
      [ -e "$d" ] && { echo "STALE $d"; exit 1; }
      mkdir -p "$d"
      args=(conformance "std/protocol/$name.ignite" --target "$target" --out "$d/out" --target-dir "$d/target" --runtime "$RT")
      timed "conformance-$name-$target-cold" after "$i" "$AFTER_SRC" env SCCACHE_RECACHE=1 "$AFTER_BIN" "${args[@]}"
      timed "conformance-$name-$target-warm" after "$i" "$AFTER_SRC" "$AFTER_BIN" "${args[@]}"
      (cd "$AFTER_SRC" && cargo clean --target-dir "$d/target" > "$X/clean-$name-$target-$i.log" 2>&1); echo "CLEAN $name $target $i exit=$?"
      rm -rf "$d/out"
    done
  done
done
for i in 1 2 3 4 5; do
  timed rfc-import-9110 after "$i" "$AFTER_SRC" "$AFTER_BIN" rfc import docs/rfc/rfc9110.xml \
    --sections 5,6,7.2,9.1,9.2,9.3.1,9.3.2,9.3.3,9.3.5 --out "$X/rfc9110-$i.ignite"
done
cmp "$X/rfc9110-1.ignite" "$AFTER_SRC/std/rfc/rfc9110.ignite" > /dev/null; echo "IMPORT_DIFFERS_FROM_COMMITTED=$? (the committed file carries the hand review of parties)"
