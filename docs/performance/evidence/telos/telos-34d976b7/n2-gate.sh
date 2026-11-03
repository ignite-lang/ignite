#!/bin/zsh
ulimit -c 0
cd /home/timo/projects/telos
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-j CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_DEBUG=0
step() {
  echo "=== $*"
  "$@" 2>&1 | grep -E '^test result|FAILED|panicked|^error|^warning|^Diff in|failures:' | head -40
  echo "STATUS=${pipestatus[1]}"
}
step cargo fmt -p telos-rustgen --check
step cargo clippy -p telos-rustgen --all-targets -- -D warnings
step cargo test -p telos-rustgen
step cargo build --workspace --all-targets
step cargo test -p telos
echo EXIT=done
