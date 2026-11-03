#!/bin/zsh
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
cd /home/timo/projects/telos
export CARGO_TARGET_DIR=$S/target-k CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
out=$S/s10p1-gate.log
: > $out
step() {
  echo "=== STEP $1" >> $out
  shift
  "$@" >> $out 2>&1
  echo "STEP_EXIT=$?" >> $out
}
step frontend-tests cargo test -p telos-syntax -p telos-check -p telos-types
step workspace-build cargo build --workspace --all-targets
step clippy cargo clippy -p telos-syntax -p telos-check -p telos-types -p telos --all-targets -- -D warnings
step fmt cargo fmt --all --check
step cli-tests cargo test -p telos --test main cli::check
step release-cli cargo build --release -p telos --bin telos
echo "EXIT=0" >> $out
