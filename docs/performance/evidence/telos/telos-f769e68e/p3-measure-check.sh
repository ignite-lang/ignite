#!/bin/bash
set -u
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
BEFORE=$S/s12-before
AFTER=/home/timo/projects/telos
BEFORE_BIN=$S/target-s12-before/debug/ignite
AFTER_BIN=$S/target-s12-p3/debug/ignite

run_check() {
  local bin=$1 tree=$2 label=$3 t0 t1 rc
  t0=$(date +%s%N)
  "$bin" check "$tree/examples/terminal/terminal.design.ignite" --std "$tree/std" >/dev/null 2>&1
  rc=$?
  t1=$(date +%s%N)
  echo "check $label $(( (t1 - t0) / 1000000 )) ms rc=$rc"
}

for i in $(seq 1 10); do
  run_check "$BEFORE_BIN" "$BEFORE" before
  run_check "$AFTER_BIN" "$AFTER" after
done
echo "std lines before: $(cat "$BEFORE"/std/*.ignite "$BEFORE"/std/*/*.ignite | wc -l) after: $(cat "$AFTER"/std/*.ignite "$AFTER"/std/*/*.ignite | wc -l)"
echo EXIT=0
