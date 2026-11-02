#!/bin/zsh
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
cd /home/timo/projects/telos
export CARGO_TARGET_DIR=$S/target-k CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
out=$S/s10p1-gate3.log
: > $out
step() {
  echo "=== STEP $1" >> $out
  shift
  "$@" >> $out 2>&1
  echo "STEP_EXIT=$?" >> $out
}
step fmt-apply cargo fmt -p telos-syntax -p telos-check -p telos-types -p telos
step frontend-tests cargo test -p telos-syntax -p telos-check -p telos-types
step clippy cargo clippy -p telos-syntax -p telos-check -p telos-types --all-targets -- -D warnings
step fmt cargo fmt -p telos-syntax -p telos-check -p telos-types -p telos --check
step workspace-build cargo build --workspace --all-targets
step cli-tests cargo test -p telos --test main cli::check
echo "EXIT=0" >> $out
