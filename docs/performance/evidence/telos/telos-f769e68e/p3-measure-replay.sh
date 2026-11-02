#!/bin/bash
set -u
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
BIN=$S/target-s12-p3/debug/ignite
TREE=/home/timo/projects/telos
BUILD=$S/target-s12-p3/tmp/run-build-design-rust
CARGO=$S/target-s12-p3/tmp/run-cargo
OUT=$S/p3-measure
mkdir -p "$OUT"
STATE=$OUT/run-state-replay
ADVANCE=${1:-5900}

if [ ! -f "$STATE/journal.jsonl" ]; then
  rm -rf "$STATE"
  FIFO=$OUT/in-replay
  rm -f "$FIFO"
  mkfifo "$FIFO"
  LOG=$OUT/run-replay.jsonl
  : > "$LOG"
  t0=$(date +%s%N)
  "$BIN" run "$TREE/examples/terminal/terminal.design.ignite" --assembly local --state "$STATE" --build "$BUILD" --target-dir "$CARGO" --std "$TREE/std" --json --clock manual --settle "1 s" --port Terminal.ingress=0 < "$FIFO" > "$LOG" 2> "$OUT/run-replay.err" &
  PID=$!
  exec 3>"$FIFO"
  until grep -q '"variant":"Ready"' "$LOG" 2>/dev/null; do
    kill -0 "$PID" 2>/dev/null || break
    sleep 0.005
  done
  echo "advance $ADVANCE seconds" >&3
  echo status >&3
  until grep -q '"kind":"status"' "$LOG" 2>/dev/null; do
    kill -0 "$PID" 2>/dev/null || break
    sleep 0.05
  done
  t1=$(date +%s%N)
  echo "journal written in $(( (t1 - t0) / 1000000 )) ms"
  echo stop >&3
  wait "$PID"
  echo "  exit=$?"
  exec 3>&-
  rm -f "$FIFO"
fi
echo "records: $(wc -l < "$STATE/journal.jsonl") bytes: $(wc -c < "$STATE/journal.jsonl")"

for i in 1 2 3 4 5; do
  t0=$(date +%s%N)
  "$BIN" replay "$TREE/examples/terminal/terminal.design.ignite" --assembly local --state "$STATE" --std "$TREE/std" > "$OUT/replay-$i.out" 2> "$OUT/replay-$i.err"
  rc=$?
  t1=$(date +%s%N)
  echo "replay $i: $(( (t1 - t0) / 1000000 )) ms rc=$rc load=$(cut -d' ' -f1 /proc/loadavg)"
done
echo EXIT=0
