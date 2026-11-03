#!/usr/bin/env bash
# usage: u3-tt.sh [rust|llvm|both] FILE.. [-- runner args]
set -u
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
K=$S/target-k
export CARGO_TARGET_DIR=$K CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
cd /home/timo/projects/telos
which=$1; shift
cargo build -q -p telos || exit 2
cargo build -q -p telos-rt --features runner --target-dir $K/rt || exit 2
if [ "$which" != llvm ]; then
  echo "== rust"
  $K/debug/telos test "$@" --out $S/u3-out --target-dir $S/u3-out/cargo
  echo "rust exit $?"
fi
if [ "$which" != rust ]; then
  echo "== llvm"
  $K/debug/telos test "$@" --target llvm --out $S/u3-out --runtime $K/rt/debug/libtelos_rt.a
  echo "llvm exit $?"
fi
