#!/usr/bin/env bash
set -u
P=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/perf15
S=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/s15
T=/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-s15-claims
bash "$S/u6c-extra.sh"
echo "EXTRA_EXIT=$?"
for b in "$P"/builds/baseline-296f84973552 "$P"/builds/candidate-c20f04b6fead "$P"/builds/programs; do
  [ -d "$b" ] || continue
  for d in $(find "$b" -maxdepth 3 -name CACHEDIR.TAG -printf '%h\n' | sort -r); do
    (cd "$T" && cargo clean --target-dir "$d" > /dev/null 2>&1); echo "CLEAN $d exit=$?"
  done
done
du -sh "$P"
cd "$T"
echo "HEAD $(git rev-parse --short HEAD)"
cargo build -q -p ignite
cargo build -q -p ignite-rt --features runner --target-dir target/ignite-test/rt
for name in telnet http; do
  echo "ROW6 $(date -u +%FT%TZ) $name"
  target/debug/ignite test std/protocol/$name.ignite --target all --runtime target/ignite-test/rt/debug/libignite_rt.a 2>&1 | grep -E 'test result|FAILED|^==|target'
  echo "ROW6_EXIT $name=${PIPESTATUS[0]}"
done
echo "ROW8 $(date -u +%FT%TZ)"
target/debug/ignite check std/rfc.ignite > "$S/row8.out" 2> "$S/row8.err"; echo "ROW8_EXIT=$? stdout=$(wc -c < "$S/row8.out") stderr=$(wc -c < "$S/row8.err")"
echo "ROW24 $(date -u +%FT%TZ)"
cargo tree -p ignite -e normal 2>/dev/null | grep -E -c '\b(reqwest|ureq|hyper|curl|isahc|attohttpc)\b'; echo "ROW24 matches above (0 means none)"
echo "GATE LOAD $(cut -d' ' -f1 /proc/loadavg) START $(date -u +%FT%TZ) $(date +%s)"
task gate
echo "GATE_EXIT=$? END $(date -u +%FT%TZ) $(date +%s) LOAD $(cut -d' ' -f1 /proc/loadavg)"
