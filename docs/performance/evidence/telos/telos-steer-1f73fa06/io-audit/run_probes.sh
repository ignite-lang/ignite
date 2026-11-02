#!/bin/zsh
set -u
S=/home/timo/.cache/scratch/1f73fa06-e945-4828-ae60-02b7cdacb054/io-audit
T=/home/timo/projects/telos/target/debug/telos
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
cd "$S" || exit 1
for f in "$S"/p*.telos; do
  name=$(basename "$f" .telos)
  echo "=== $name build"
  "$T" build "$f" --out "$S/out/$name" --target-dir "$S/cargo" > "$S/$name.build.log" 2>&1
  echo "EXIT=$?"
  grep -E '^error|^warning: unused|E0[0-9]{3}|error\[' "$S/$name.build.log" | head -12
  if grep -q '^test ' "$f"; then
    echo "=== $name test"
    "$T" test "$f" --out "$S/t/$name" --target-dir "$S/cargo" --timeout 10 > "$S/$name.test.log" 2>&1
    echo "EXIT=$?"
    grep -E 'test result|FAILED|^error|error\[|panic|failed|deadlock|timed out' "$S/$name.test.log" | head -12
  fi
done
echo "EXIT=ALLDONE"
