#!/bin/sh
cd /home/timo/projects/telos || exit 1
for f in "$@"; do
  echo "===== $(basename "$f")"
  target/debug/telos check "$f" 2>&1
  echo "EXIT=$?"
done
