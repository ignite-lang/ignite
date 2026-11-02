#!/bin/zsh
set -u
cd /home/timo/projects/telos
B=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-h
TELOS="$B/debug/telos"
SIB=/home/timo/.cache/scratch/1f73fa06-e945-4828-ae60-02b7cdacb054/sibling
LOG=/home/timo/.cache/scratch/1f73fa06-e945-4828-ae60-02b7cdacb054/sibling.log
ulimit -c 0
RT=$(ls -t "$B"/debug/deps/libtelos_rt-*.a | head -1)
{
  echo "RT=$RT"
  echo "=== check ==="
  "$TELOS" check "$SIB/demo.telos"; echo "check EXIT=$?"
  echo "=== build --target llvm ==="
  "$TELOS" build --target llvm "$SIB/demo.telos" --out "$SIB/out-build" --runtime "$RT"
  echo "build EXIT=$?"
  echo "=== nm for demo.ops.greeting ==="
  nm "$SIB/out-build/bin/demo" 2>/dev/null | grep 'demo.ops.greeting' || echo "(symbol not found by nm)"
  echo "=== run built client against a listener ==="
  python3 -I /home/timo/.cache/scratch/1f73fa06-e945-4828-ae60-02b7cdacb054/listener_read.py "$SIB/out-build/bin/demo"
  echo "=== test --target llvm ==="
  "$TELOS" test --target llvm "$SIB/demo.telos" --out "$SIB/out-test" --runtime "$RT"
  echo "test EXIT=$?"
} > "$LOG" 2>&1
