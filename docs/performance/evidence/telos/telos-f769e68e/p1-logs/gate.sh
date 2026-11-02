#!/usr/bin/zsh
set -u
cd /home/timo/projects/telos || exit 1
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
export CARGO_TARGET_DIR=$S/target-p1 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0
ulimit -c 0
df -h / | tail -1
echo "== fmt"
cargo fmt --all -- --check 2>&1 | tail -20
echo "fmt rc=${pipestatus[1]}"
echo "== clippy"
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -E '^(error|warning)|-->|Finished|could not compile' | head -60
echo "clippy rc=${pipestatus[1]}"
echo "== build"
cargo build --workspace --all-targets 2>&1 | grep -E '^(error|warning)|-->|Finished|could not compile' | head -40
echo "build rc=${pipestatus[1]}"
echo "== test frontend"
cargo test -p ignite-syntax -p ignite-check -p ignite-types -p ignite-eval 2>&1 | grep -E '^test result|FAILED|panicked|Running' | head -40
echo "test rc=${pipestatus[1]}"
echo "== check files"
rc=0
for f in std/**/*.ignite examples/**/*.ignite; do
  out=$($S/target-p1/debug/ignite check "$f" 2>&1); r=$?
  if [[ $r -ne 0 ]]; then echo "FAIL $f rc=$r"; echo "$out" | head -5; rc=1; else echo "ok   $f"; fi
done
echo "check rc=$rc"
echo "EXIT=0"
