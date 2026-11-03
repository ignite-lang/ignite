#!/usr/bin/env zsh
cd /home/timo/projects/telos || exit 1
ulimit -c 0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0 CARGO_PROFILE_RELEASE_DEBUG=0
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-main
start=$(date +%s)
task gate
rc=$?
echo "WALL=$(( $(date +%s) - start ))s"
echo "EXIT=$rc"
