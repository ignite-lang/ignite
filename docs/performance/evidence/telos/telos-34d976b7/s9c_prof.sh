#!/bin/zsh
# usage: s9c_prof.sh LOG TESTBIN
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
ulimit -c 0
cd /home/timo/projects/telos/crates/telos
LOG=$1; BIN=$2
{
  for i in; do
    t0=$(date +%s.%N)
    $BIN --exact slice9::long_chains_lower_quickly > /dev/null 2>&1
    echo "plain run $i: $(( $(date +%s.%N) - t0 )) s status $?"
  done
  perf record -F 300 --call-graph dwarf,32768 -e cpu-clock:u -o $S/s9c-prof.data -- $BIN --exact slice9::long_chains_lower_quickly > /dev/null 2>&1
  perf report -i $S/s9c-prof.data --stdio --no-children --sort sym 2>/dev/null | grep -E '^ +[0-9.]+%' | head -25
  echo "--- children"
  perf report -i $S/s9c-prof.data --stdio --children --sort sym -g none 2>/dev/null | grep -E '^ +[0-9.]+%' | head -300
  echo "EXIT=0"
} > $LOG 2>&1
