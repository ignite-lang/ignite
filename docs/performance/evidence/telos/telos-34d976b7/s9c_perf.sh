#!/bin/zsh
# usage: s9c_perf.sh LOG TODOD PORT
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
ulimit -c 0
LOG=$1; BIN=$2; PORT=$3
{
  perf record -F 4999 -e cpu-clock:u -o $S/s9c-perf.data -- $BIN --port $PORT > $LOG.srv 2>&1 &
  P=$!
  echo "perf pid $P"
  n=0
  until grep -q 'listening on' $LOG.srv || [ $n -gt 100 ]; do timeout 0.1 tail -f /dev/null; n=$((n+1)); done
  C=$(cat /proc/$P/task/$P/children); C=${C// /}
  echo "todod pid $C"
  urls=()
  for i in {1..200}; do urls+=(http://127.0.0.1:$PORT/todos); done
  for r in {1..10}; do curl -s -o /dev/null --max-time 20 -X POST -d '{"title": "milk and more milk"}' $urls; done
  for r in {1..20}; do curl -s -o /dev/null --max-time 20 http://127.0.0.1:$PORT/todos; done
  kill -INT $C
  wait $P
  echo "perf exit $?"
  perf report -i $S/s9c-perf.data --stdio --no-children --sort sym,srcline 2>/dev/null | grep -v '^#' | grep -v '^$' | head -25
  echo "EXIT=0"
} > $LOG 2>&1
