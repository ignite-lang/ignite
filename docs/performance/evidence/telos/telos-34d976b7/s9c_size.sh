#!/bin/zsh
# usage: s9c_size.sh LOG OPT -- the todod of s9c-todo-now rebuilt the pre-S9C way: IR without debug metadata, -O1, no -g
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
ulimit -c 0
IN=$S/s9c-todo-now/llvm
OUT=$S/s9c-todo-before
LOG=$1; OPT=$2
{
  rm -rf $OUT; mkdir -p $OUT
  for u in std program main_todod; do
    opt -S -strip-debug $IN/$u.ll -o $OUT/$u.ll || exit 1
    clang -c $OPT -ffunction-sections -fdata-sections --target=x86_64-pc-linux-gnu $OUT/$u.ll -o $OUT/$u.o || exit 1
  done

  args=()
  for a in $(sed -n "1,/^-lc$/p" $IN/todod.link.stamp); do
    case $a in
      */s9c-todo-now/bin/todod) args+=($OUT/todod) ;;
      */s9c-todo-now/llvm/*.o) args+=($OUT/${a:t}) ;;
      *) args+=($a) ;;
    esac
  done
  echo "${args[@]}"
  "${args[@]}"
  echo "link=$?"
  ls -l $OUT/todod
  size -A $OUT/todod | grep -E '^\.text|Total'
  echo "EXIT=0"
} > $LOG 2>&1
