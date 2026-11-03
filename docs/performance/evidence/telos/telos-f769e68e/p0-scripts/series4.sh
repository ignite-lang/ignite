#!/bin/zsh
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p0-scripts/env.sh
prog=$LOGS/series4.progress
: >$prog
nowait() {
  echo "$(date -Is) start $* load=$(load1)" >>$prog
  zsh $S/p0-scripts/"$@"
  echo "$(date -Is) end $* load=$(load1)" >>$prog
}
nowait todod.sh 1
nowait gate.sh warm 4
nowait perfile.sh 1
nowait todod.sh 2
nowait clang-steps.sh 1 3
nowait gate.sh warm 5
nowait perfile.sh 2
nowait todod.sh 3
nowait gate.sh warm 6
nowait perfile.sh 3
nowait pertest.sh 1 terminal todo llvm_build llvm_memory failover slice9 syntax cli ignite_test llvm siblings slice10_llvm generics
echo "EXIT=0" >>$prog
