#!/bin/bash
set -u
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
BIN=$S/target-s12-p3/debug/ignite
TREE=/home/timo/projects/telos
BUILD=$S/target-s12-p3/tmp/run-build-design-rust
CARGO=$S/target-s12-p3/tmp/run-cargo
OUT=$S/p3-measure
mkdir -p "$OUT"

for i in 0 1 2 3 4 5; do
  STATE=$OUT/run-state-ready-$i
  rm -rf "$STATE"
  FIFO=$OUT/in-ready-$i
  rm -f "$FIFO"
  mkfifo "$FIFO"
  LOG=$OUT/run-ready-$i.jsonl
  : > "$LOG"
  t0=$(date +%s%N)
  "$BIN" run "$TREE/examples/terminal/terminal.design.ignite" --assembly local --state "$STATE" --build "$BUILD" --target-dir "$CARGO" --std "$TREE/std" --json --clock manual --settle "1 s" --port Terminal.ingress=0 < "$FIFO" > "$LOG" 2> "$OUT/run-ready-$i.err" &
  PID=$!
  exec 3>"$FIFO"
  until grep -q '"variant":"Ready"' "$LOG" 2>/dev/null; do
    kill -0 "$PID" 2>/dev/null || break
    sleep 0.005
  done
  t1=$(date +%s%N)
  echo "run $i: ready after $(( (t1 - t0) / 1000000 )) ms load=$(cut -d' ' -f1 /proc/loadavg)"
  echo stop >&3
  wait "$PID"
  echo "  exit=$? records=$(wc -l < "$STATE/journal.jsonl")"
  exec 3>&-
  rm -f "$FIFO"
done
echo EXIT=0
