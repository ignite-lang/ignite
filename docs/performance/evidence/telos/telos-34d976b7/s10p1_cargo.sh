#!/bin/zsh
# usage: s10p1_cargo.sh <outfile> <cargo args...>
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
cd /home/timo/projects/telos
export CARGO_TARGET_DIR=$S/target-k CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
out=$1
shift
cargo "$@" > $out 2>&1
st=$?
echo "EXIT=$st" >> $out
