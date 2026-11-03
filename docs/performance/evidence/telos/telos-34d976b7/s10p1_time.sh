#!/bin/zsh
# usage: s10p1_time.sh <telos binary> <label>
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
cd /home/timo/projects/telos
bin=$1
for f in examples/todo/todo.telos $S/s10p1-chains/chains.telos; do
  for i in 1 2 3 4 5; do
    s=$(date +%s.%N)
    $bin check $f > /dev/null 2>&1
    st=$?
    e=$(date +%s.%N)
    printf "%s %s run%d status=%d %.3f\n" $2 $f $i $st $(( e - s ))
  done
done
