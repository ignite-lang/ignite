#!/bin/bash
set -u
ulimit -c 0
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
TREE=/home/timo/projects/telos
LOGS=$S/p3-gate-logs
mkdir -p "$LOGS"
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_DEBUG=0

step() {
  local name=$1
  shift
  local t0 t1 rc
  t0=$(date +%s%N)
  "$@" > "$LOGS/$name.log" 2>&1
  rc=$?
  t1=$(date +%s%N)
  echo "step $name rc=$rc $(( (t1 - t0) / 1000000 )) ms load=$(cut -d' ' -f1 /proc/loadavg)"
}

cd "$TREE" || exit 1
export CARGO_TARGET_DIR=$S/target-s12-p3
step fmt cargo fmt --check -p ignite -p ignite-eval
step clippy cargo clippy --workspace --all-targets -- -D warnings
step build-all cargo build --workspace --all-targets
step test-eval cargo test -p ignite-eval
step test-ignite cargo test -p ignite
step test-run-file cargo test -p ignite --test main -- run::
step test-control-trace-file cargo test -p ignite --test main -- control_trace::
step test-decode-file cargo test -p ignite --test main -- decode::
step check-design cargo test -p ignite --test main -- cli::check_design_file_is_silent

cd "$S/s12-before" || exit 1
export CARGO_TARGET_DIR=$S/target-s12-before
step build-before cargo build -p ignite --locked

cd "$TREE" || exit 1
export CARGO_TARGET_DIR=$S/target-s12-p3
step measure-plan "$S/p3-measure-plan.sh"
step measure-run "$S/p3-measure-run.sh"
step measure-replay "$S/p3-measure-replay.sh"
echo EXIT=0
