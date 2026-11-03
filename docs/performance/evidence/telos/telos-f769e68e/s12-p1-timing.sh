#!/bin/sh
# Interleaved pairs (15.1): before tree, then after tree, per command; load per run.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
BEFORE_BIN=$S/target-s12-p1-before/debug/ignite
AFTER_BIN=$S/target-s12-p1/debug/ignite
BEFORE_DIR=$S/s12-before
AFTER_DIR=/home/timo/projects/telos
FILE=examples/terminal/terminal.design.ignite
PAIRS=${PAIRS:-4}
now() { date +%s.%N; }
load() { cut -d' ' -f1 /proc/loadavg; }
run() {
  dir=$1; bin=$2; cmd=$3
  cd "$dir" || exit 9
  l=$(load)
  t0=$(now)
  if [ "$cmd" = check ]; then
    "$bin" check "$FILE" > /dev/null 2>&1; rc=$?
  else
    "$bin" plan "$FILE" > /dev/null 2>&1; rc=$?
  fi
  t1=$(now)
  printf '%s %s load=%s time=%.3f rc=%s\n' "$4" "$cmd" "$l" "$(echo "$t1 - $t0" | bc)" "$rc"
}
"$BEFORE_BIN" check "$BEFORE_DIR/$FILE" > /dev/null 2>&1
"$AFTER_BIN" check "$AFTER_DIR/$FILE" > /dev/null 2>&1
i=1
while [ "$i" -le "$PAIRS" ]; do
  for cmd in check plan; do
    run "$BEFORE_DIR" "$BEFORE_BIN" "$cmd" "pair$i before"
    run "$AFTER_DIR" "$AFTER_BIN" "$cmd" "pair$i after"
  done
  i=$((i + 1))
done
echo "EXIT=0"
