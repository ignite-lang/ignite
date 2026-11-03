#!/bin/zsh
# usage: s9c_opt.sh LOG
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-h CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
cd /home/timo/projects/telos
LOG=$1
FILES=($(find std examples -name '*.telos' ! -name '*.design.telos' | sort))
{
  for opt in O0 O1 O0 O1; do
    if [[ $opt == O1 ]]; then CL=$S/s9c_clang_o1.sh; else CL=clang; fi
    OUT=$S/s9c-opt-$opt
    rm -rf $OUT
    for pass in fresh cached; do
      t0=$(date +%s.%N)
      $S/target-h/debug/telos test $FILES --target llvm --clang $CL --out $OUT --runtime $S/target-h/rt/debug/libtelos_rt.a > $LOG.$opt.$pass 2>&1
      st=$?
      t1=$(date +%s.%N)
      echo "$opt $pass status=$st wall=$(( t1 - t0 )) $(grep -E 'test result' $LOG.$opt.$pass | tr '\n' ' ')"
    done
  done
  echo "EXIT=0"
} > $LOG 2>&1
