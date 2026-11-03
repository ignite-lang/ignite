#!/bin/zsh
# usage: s9c_compile.sh LOG -- task compile-llvm and compile-todo-llvm with outputs in scratch
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-h CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
cd /home/timo/projects/telos
{
  task compile-llvm OUT_LLVM=$S/s9c-compile/terminal
  a=$?
  task compile-todo-llvm TODO_OUT_LLVM=$S/s9c-compile/todo
  b=$?
  ls -l $S/s9c-compile/terminal/bin $S/s9c-compile/todo/bin
  echo "compile-llvm=$a compile-todo-llvm=$b"
  echo "EXIT=$(( a + b ))"
} > $1 2>&1
