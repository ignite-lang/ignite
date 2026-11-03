#!/bin/zsh
# usage: s9c_gdb.sh LOG TODOD PORT
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
ulimit -c 0
LOG=$1; BIN=$2; PORT=$3
{
  gdb -batch -nx \
    -ex 'set pagination off' \
    -ex 'break todo.telos:28' \
    -ex "run --port $PORT" \
    -ex 'bt 4' \
    -ex 'info source' \
    -ex 'kill' \
    $BIN > $LOG.gdb 2>&1 &
  G=$!
  echo "gdb pid $G"
  n=0
  until grep -q 'listening on' $LOG.gdb || [ $n -gt 100 ]; do timeout 0.1 tail -f /dev/null; n=$((n+1)); done
  curl -s --max-time 5 -X POST -d '{"title": "milk"}' http://127.0.0.1:$PORT/todos
  echo "curl exit $?"
  wait $G
  echo "gdb exit $?"
  cat $LOG.gdb
  echo "EXIT=0"
} > $LOG 2>&1
