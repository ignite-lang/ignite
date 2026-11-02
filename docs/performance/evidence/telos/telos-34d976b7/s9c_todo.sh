#!/bin/zsh
# usage: s9c_todo.sh LOG OUTDIR [extra telos build args]
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-h CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
cd /home/timo/projects/telos
LOG=$1; OUT=$2; shift 2
{
  cargo build -q -p telos && cargo build -q -p telos-rt --features runner --target-dir $S/target-h/rt && \
  T0=$(date +%s.%N); $S/target-h/debug/telos build examples/todo/todo.telos --target llvm --out $OUT --runtime $S/target-h/rt/debug/libtelos_rt.a "$@"
  R=$?; echo "wall $(( $(date +%s.%N) - T0 )) s"; echo "EXIT=$R"
} > $LOG 2>&1
