#!/usr/bin/env bash
set -u
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
K=$S/target-k
T=$K/debug/telos
EX=$S/snap3/examples/terminal/terminal.telos
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
unset CARGO_TARGET_DIR
cd $S/snap3
rc=0
$T build $EX --out $K/srv/rust-release --target-dir $K/srv/rust-target --release || rc=1
$T build $EX --out $K/srv/rust-debug --target-dir $K/srv/rust-target || rc=1
$T build $EX --target llvm --release --runtime $K/release/libtelos_rt.a --out $K/srv/llvm-release || rc=1
$T build $EX --target llvm --runtime $K/debug/libtelos_rt.a --out $K/srv/llvm-debug || rc=1
echo EXIT=$rc
