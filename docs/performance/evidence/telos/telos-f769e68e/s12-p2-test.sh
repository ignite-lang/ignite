#!/bin/zsh
# P2 slice 12: cargo test of the crates named in $@ ; the log ends with EXIT=<code>.
set -o pipefail
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
export CARGO_TARGET_DIR=$S/target-s12-p2
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
ulimit -c 0
cd /home/timo/projects/telos || exit 9
df -h / | tail -1
cargo test "$@" 2>&1 | grep -v '^\s*Compiling\|^\s*Blocking\|^\s*Locking\|^\s*Finished\|^test .* ok$\|^\s*$'
echo "EXIT=$?"
