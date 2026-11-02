#!/bin/zsh
# P4: the four task compile targets, then cold (SCCACHE_RECACHE=1) todod builds, two runs each.
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-k
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_INCREMENTAL=0
cd /home/timo/projects/telos || exit 1
LOG=$S/p4_compile2.log
RT=$CARGO_TARGET_DIR/telos-test/rt/debug/libtelos_rt.a
PROG=$S/gate-programs
TELOS=$CARGO_TARGET_DIR/debug/telos
now() { date +%s.%N }
{
  echo "START $(date +%T)"
  ulimit -c 0
  uptime
  for t in compile compile-llvm compile-todo compile-todo-llvm; do
    t0=$(now)
    task $t 2>&1 | tail -4
    echo "task $t EXIT=${pipestatus[1]} $(printf '%.1f' $(( $(now) - t0 ))) s"
  done
  ls -la target/debug/terminald target/debug/tn target/telos-llvm/terminal/bin/ target/debug/todod target/telos-llvm/todo/bin/ 2>&1 | grep -v '^total'
  export SCCACHE_RECACHE=1
  for run in 1 2; do
    for profile in dev release; do
      flag=""
      [[ $profile == release ]] && flag="--release"
      rm -rf $PROG/cold-$profile-$run
      t=$(now); $TELOS build examples/todo/todo.telos --out $PROG/cold-$profile-$run --target-dir $PROG/cold-$profile-$run/target $flag > /dev/null 2>&1; echo "todod rust $profile cold run $run EXIT=$? $(printf '%.1f' $(( $(now) - t ))) s"
    done
  done
  unset SCCACHE_RECACHE
  for run in 1 2; do
    for profile in dev release; do
      flag=""
      [[ $profile == release ]] && flag="--release"
      rm -rf $PROG/llvm-$profile-$run
      t=$(now); $TELOS build examples/todo/todo.telos --target llvm --runtime $RT --out $PROG/llvm-$profile-$run $flag > /dev/null 2>&1; echo "todod llvm $profile run $run EXIT=$? $(printf '%.1f' $(( $(now) - t ))) s"
    done
  done
  wc -l $PROG/cold-release-1/std/src/lib.rs $PROG/cold-release-1/src/lib.rs $PROG/cold-release-1/src/bin/*.rs
  uptime
  echo "END $(date +%T)"
  echo "EXIT=0"
} > "$LOG" 2>&1
