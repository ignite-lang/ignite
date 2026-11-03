#!/bin/zsh
# P2 slice 12: build the ignite CLI (all targets) and the runner runtime; the log ends with EXIT=<code>.
set -o pipefail
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
export CARGO_TARGET_DIR=$S/target-s12-p2
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
cd /home/timo/projects/telos || exit 9
df -h / | tail -1
cargo build -p ignite --all-targets 2>&1 | grep -v '^\s*Compiling\|^\s*Blocking\|^\s*Locking' | tail -60
code=$?
if [ $code -eq 0 ]; then
  cargo build -p ignite-rt --features runner --target-dir $S/target-s12-p2/ignite-test/rt 2>&1 | grep -v '^\s*Compiling\|^\s*Blocking\|^\s*Locking' | tail -20
  code=$?
fi
echo "EXIT=$code"
