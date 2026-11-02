#!/bin/sh
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-j CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_DEBUG=0
TAG=$1
cd /home/timo/projects/telos || exit 1
ulimit -c 0
cargo build -q --release -p telos-rt || exit 1
cargo build -q -p telos || exit 1
OUT=$S/u2-rel-$TAG
rm -rf $OUT
mkdir -p $OUT
cp $CARGO_TARGET_DIR/release/libtelos_rt.a $OUT/
$CARGO_TARGET_DIR/debug/telos build examples/terminal/terminal.telos --target llvm --release --runtime $OUT/libtelos_rt.a --out $OUT/llvm > $OUT/llvm.log 2>&1
echo "llvm=$?"
stat -c '%s %n' $OUT/llvm/bin/terminald $OUT/llvm/bin/tn
echo EXIT=0
