#!/bin/zsh
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p0-scripts/env.sh
prog=$LOGS/series3.progress
: >$prog
unit() {
  echo "$(date -Is) wait load<4.5 for $* (load=$(load1))" >>$prog
  waitload 4.5 1500
  echo "$(date -Is) start $* load=$(load1)" >>$prog
  zsh $S/p0-scripts/"$@"
  echo "$(date -Is) end $* load=$(load1)" >>$prog
}
nowait() {
  echo "$(date -Is) start $* load=$(load1)" >>$prog
  zsh $S/p0-scripts/"$@"
  echo "$(date -Is) end $* load=$(load1)" >>$prog
}
nowait flake.sh alone 30
nowait flake.sh beside 30
unit todod.sh 1
unit gate.sh warm 4
unit perfile.sh 1
unit todod.sh 2
unit clang-steps.sh 1 3
unit gate.sh warm 5
unit perfile.sh 2
unit todod.sh 3
unit perfile.sh 3
unit pertest.sh 1 terminal todo llvm_build llvm_memory failover slice9 syntax cli ignite_test llvm siblings slice10_llvm generics
echo "EXIT=0" >>$prog
