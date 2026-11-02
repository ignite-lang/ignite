#!/usr/bin/env bash
set -u
cd /home/timo/projects/telos
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
T=$S/target-i/debug/telos
W=$S/m-${1:-x}
mkdir -p "$W"
EX=examples/terminal/terminal.telos
for p in debug release; do
  flag=""
  [ $p = release ] && flag=--release
  rm -rf "$W/$p"
  s=$(date +%s.%N)
  $T build $EX --target llvm --out "$W/$p" --runtime "$S/target-i/$p/libtelos_rt.a" $flag > "$W/$p.log" 2>&1 || { cat "$W/$p.log"; exit 1; }
  e=$(date +%s.%N)
  s2=$(date +%s.%N)
  $T build $EX --target llvm --out "$W/$p" --runtime "$S/target-i/$p/libtelos_rt.a" $flag > "$W/$p.log" 2>&1
  e2=$(date +%s.%N)
  for x in terminald tn; do
    f=$W/$p/bin/$x
    strip -o "$W/$p-$x.s" "$f"
    printf '%s %s size=%s stripped=%s build=%.2f rebuild=%.2f\n' $p $x $(stat -c %s "$f") $(stat -c %s "$W/$p-$x.s") $(echo "$e - $s" | bc) $(echo "$e2 - $s2" | bc)
  done
done
