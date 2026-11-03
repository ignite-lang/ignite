#!/bin/sh
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-j CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_DEBUG=0
O=$S/u2-gate
cd /home/timo/projects/telos || exit 1
ulimit -c 0
cargo fmt --all -- --check > $O.fmt 2>&1; echo "FMT=$?"
cargo clippy --workspace --all-targets -- -D warnings > $O.clippy 2>&1; echo "CLIPPY=$?"
cargo clippy -p telos-io -p telos-rt --all-targets --features runner -- -D warnings > $O.clippy-runner 2>&1; echo "CLIPPY_RUNNER=$?"
cargo test --workspace > $O.test 2>&1; echo "TEST=$?"
FILES=$(find std examples -name '*.telos' ! -name '*.design.telos' | sort | tr '\n' ' ')
cargo build -q -p telos > $O.tt 2>&1
cargo build -q -p telos-rt --features runner --target-dir $S/u2-tt-rt >> $O.tt 2>&1; echo "RT_RUNNER=$?"
$CARGO_TARGET_DIR/debug/telos test $FILES --out $S/u2-tt-out --target-dir $S/u2-tt-out/cargo > $O.tt-rust 2>&1; echo "TT_RUST=$?"
$CARGO_TARGET_DIR/debug/telos test $FILES --target llvm --out $S/u2-tt-out --runtime $S/u2-tt-rt/debug/libtelos_rt.a > $O.tt-llvm 2>&1; echo "TT_LLVM=$?"
echo EXIT=0
