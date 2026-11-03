#!/bin/zsh
# P4: the typer's terminal test, then the gate measurement, then the compile targets.
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-k
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_INCREMENTAL=0
cd /home/timo/projects/telos || exit 1
LOG=$S/p4_types_then_gate.log
{
  echo "START $(date +%T)"
  ulimit -c 0
  cargo fmt --all 2>&1; echo "fmt EXIT=$?"
  cargo test -p telos-types --test main terminal:: 2>&1 | grep -v ' \.\.\. ok$'
  TYPES=${pipestatus[1]}
  echo "types EXIT=$TYPES"
  if [[ $TYPES == 0 ]]; then
    $S/p4_gate.sh p4_gate
    echo "gate script done $(date +%T)"
    cat $S/p4_gate.log
    $S/p4_compile.sh p4_compile
    echo "compile script done $(date +%T)"
    cat $S/p4_compile.log
  fi
  echo "END $(date +%T)"
  echo "EXIT=0"
} > "$LOG" 2>&1
