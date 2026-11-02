#!/bin/sh
cd /home/timo/projects/telos || exit 9
IGNITE=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/target-s12-p1/debug/ignite
status=0
for f in $(find std -name '*.ignite' | sort) examples/terminal/terminal.ignite examples/todo/todo.ignite examples/terminal/terminal.design.ignite; do
  out=$("$IGNITE" check "$f" 2>&1)
  code=$?
  if [ "$code" -ne 0 ] || [ -n "$out" ]; then
    echo "FAIL $f exit=$code"
    echo "$out"
    status=1
  else
    echo "ok   $f"
  fi
done
echo "EXIT=$status"
