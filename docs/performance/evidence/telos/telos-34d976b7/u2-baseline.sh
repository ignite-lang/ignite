#!/bin/sh
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-j CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_DEBUG=0
TAG=${1:-base}
cd /home/timo/projects/telos || exit 1
ulimit -c 0
cargo build -q -p telos -p telos-rt || { echo "EXIT=build-failed"; exit 1; }
T=$CARGO_TARGET_DIR/debug/telos
OUT=$S/u2-$TAG
rm -rf $OUT
mkdir -p $OUT
$T build examples/terminal/terminal.telos --out $OUT/rust --target-dir $OUT/rust-target > $OUT/rust.log 2>&1
echo "rust=$?"
$T build examples/terminal/terminal.telos --target llvm --out $OUT/llvm > $OUT/llvm.log 2>&1
echo "llvm=$?"
$T emit examples/terminal/terminal.telos --out $OUT/emit > $OUT/emit.log 2>&1
echo "emit=$?"
stat -c '%s %n' $OUT/rust-target/debug/terminald $OUT/rust-target/debug/tn $OUT/llvm/bin/terminald $OUT/llvm/bin/tn
echo EXIT=0
