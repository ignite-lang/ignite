#!/bin/bash
set -u
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
BEFORE=$S/s12-before
AFTER=/home/timo/projects/telos
BEFORE_BIN=$S/target-s12-before/debug/ignite
AFTER_BIN=$S/target-s12-p3/debug/ignite
OUT=$S/p3-measure
mkdir -p "$OUT"

ns() { date +%s%N; }
load() { cut -d' ' -f1 /proc/loadavg; }

run_plan() {
  local bin=$1 tree=$2 label=$3 t0 t1 rc
  t0=$(ns)
  "$bin" plan "$tree/examples/terminal/terminal.design.ignite" --assembly local --std "$tree/std" --out "$OUT/plan-$label.json" >"$OUT/plan-$label.log" 2>&1
  rc=$?
  t1=$(ns)
  echo "plan $label $(( (t1 - t0) / 1000000 )) ms rc=$rc load=$(load)"
}

for i in $(seq 1 15); do
  run_plan "$BEFORE_BIN" "$BEFORE" before
  run_plan "$AFTER_BIN" "$AFTER" after
done
echo "before bytes: $(wc -c < "$OUT/plan-before.json")"
echo "after bytes: $(wc -c < "$OUT/plan-after.json")"
echo EXIT=0
