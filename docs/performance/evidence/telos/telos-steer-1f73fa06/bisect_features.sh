#!/bin/zsh
set -u
ulimit -c 0
cd /home/timo/projects/telos
B=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-h
S=/home/timo/.cache/scratch/1f73fa06-e945-4828-ae60-02b7cdacb054/bisect
SRC=crates/telos-rustgen/tests/slice10/features.telos
RT=$(ls -t "$B"/debug/deps/libtelos_rt-*.a | head -1)
rm -rf "$S"; mkdir -p "$S"
# header: everything before the first test block
awk '/^test "/ {exit} {print}' "$SRC" > "$S/header.telos"
# one file per test block (a block runs from its `test "` line to the next `test "` line or EOF)
awk -v dir="$S" '
  /^test "/ { n++; name=$0; sub(/^test "/,"",name); sub(/".*/,"",name); file=dir "/t" n ".telos"; print name > (dir "/t" n ".name") }
  n>0 { print >> file }
' "$SRC"
count=$(ls "$S"/t*.telos | wc -l)
for i in $(seq 1 $count); do
  f="$S/case$i.telos"
  cat "$S/header.telos" "$S/t$i.telos" > "$f"
  name=$(cat "$S/t$i.name")
  out=$("$B/debug/telos" test --target llvm "$f" --out "$S/out$i" --runtime "$RT" 2>&1)
  st=$?
  if echo "$out" | grep -q 'panicked at'; then
    verdict="PANIC: $(echo "$out" | grep -A1 'panicked at' | tail -1)"
  elif [ $st -eq 0 ]; then
    verdict="ok"
  else
    verdict="FAILED (exit $st): $(echo "$out" | grep -m1 'error')"
  fi
  printf 'TEST %d [%s]: %s\n' "$i" "$name" "$verdict"
done
echo "EXIT=0"
