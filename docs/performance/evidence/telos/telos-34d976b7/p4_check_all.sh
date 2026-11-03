#!/bin/zsh
# P4: fmt check, clippy, full test suite, test-telos; one log.
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_INCREMENTAL=0
cd /home/timo/projects/telos || exit 1
LOG=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/$1.log
{
  echo "START $(date +%T)"
  ulimit -c 0
  cargo fmt --all -- --check 2>&1; echo "fmt EXIT=$?"
  cargo clippy --workspace --all-targets -- -D warnings 2>&1 | grep -v '^\s*Compiling\|^\s*Checking' ; echo "clippy EXIT=${pipestatus[1]}"
  cargo test --workspace --no-fail-fast 2>&1 | grep -v ' \.\.\. ok$' ; echo "test EXIT=${pipestatus[1]}"
  task test-telos 2>&1 | tail -40; echo "test-telos EXIT=${pipestatus[1]}"
  echo "END $(date +%T)"
  echo "EXIT=0"
} > "$LOG" 2>&1
