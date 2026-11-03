#!/usr/bin/env bash
# usage: link.sh <dir with llvm/*.o> <runtime.a> <label> [extra flags...]
set -u
D=$1; RT=$2; L=$3; shift 3
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
O=$S/lk-$L
clang -o "$O" "$D/llvm/main_terminald.o" "$D/llvm/program.o" "$D/llvm/std.o" "$RT" -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc "$@" || exit 1
strip -o "$O.s" "$O"
printf '%-28s size=%s stripped=%s\n' "$L" $(stat -c %s "$O") $(stat -c %s "$O.s")
