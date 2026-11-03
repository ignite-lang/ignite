#!/bin/zsh
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
cd /home/timo/projects/telos || exit 2
ulimit -c 0
export CARGO_TARGET_DIR=$S/target-ign CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_DEBUG=0
step() {
  name=$1
  shift
  "$@" > $S/logs/chain1-$name.log 2>&1
  code=$?
  echo "STEP=$name EXIT=$code"
  if [ $code -ne 0 ]; then
    tail -20 $S/logs/chain1-$name.log
    echo "EXIT=$code"
    exit $code
  fi
}
step rt-test cargo test -p ignite-rt
step test-ignite task test-ignite
step compile task compile
step compile-llvm task compile-llvm
step compile-todo task compile-todo
step compile-todo-llvm task compile-todo-llvm
step iotest cargo build --manifest-path docs/audit/2026-11-01/io/iotest/Cargo.toml --target-dir $S/target-iotest
echo "EXIT=0"
