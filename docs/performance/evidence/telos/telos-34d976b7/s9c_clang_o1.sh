#!/bin/zsh
args=()
for a in "$@"; do
  if [[ $a == -O0 ]]; then args+=(-O1); else args+=("$a"); fi
done
exec clang "${args[@]}"
