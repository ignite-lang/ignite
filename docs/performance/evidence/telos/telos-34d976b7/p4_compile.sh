#!/bin/zsh
# P4: the four task compile targets, timed; one log.
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_INCREMENTAL=0
cd /home/timo/projects/telos || exit 1
LOG=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/$1.log
now() { date +%s.%N }
{
  echo "START $(date +%T)"
  ulimit -c 0
  for t in compile compile-llvm compile-todo compile-todo-llvm; do
    t0=$(now)
    task $t 2>&1 | tail -5
    echo "task $t EXIT=${pipestatus[1]} $(printf '%.1f' $(( $(now) - t0 ))) s"
  done
  ls -la target/debug/terminald target/debug/tn target/telos-llvm/terminal/bin/ target/debug/todod target/telos-llvm/todo/bin/ 2>&1 | grep -v '^total'
  echo "END $(date +%T)"
  echo "EXIT=0"
} > "$LOG" 2>&1
