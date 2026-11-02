#!/bin/zsh
P=$1
LIMIT=$2
while kill -0 $P 2>/dev/null; do
  e=$(ps -o etimes= -p $P | tr -d ' ')
  if [ -n "$e" ] && [ "$e" -gt "$LIMIT" ]; then
    kill $P
    echo "killed $P after ${e} s"
    break
  fi
  timeout 5 tail -f /dev/null
done
echo EXIT=0
