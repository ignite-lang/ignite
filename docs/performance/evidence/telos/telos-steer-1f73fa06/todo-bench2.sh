#!/bin/zsh
ulimit -c 0
B=/home/timo/.cache/scratch/1f73fa06-e945-4828-ae60-02b7cdacb054/todo-bench
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_PROFILE_RELEASE_DEBUG=0
cd "$B/tree" || exit 1
TELOS=$B/tc/debug/telos
echo "RUSTC_WRAPPER env=${RUSTC_WRAPPER:-unset}"; grep -rn 'wrapper' ~/.cargo/config.toml 2>/dev/null
sccache --show-stats 2>/dev/null | grep -E 'Compile requests|Cache hits|Cache misses|Forced recaches' | head -5

run() {
  local label=$1; shift
  local t0=$(date +%s.%N)
  "$@" > $B/$label.out 2>&1
  local st=$?
  echo "$label secs=$(printf '%.1f' $(( $(date +%s.%N) - t0 ))) status=$st compiling_lines=$(grep -c 'Compiling' $B/$label.out)"
}

export SCCACHE_RECACHE=1
run rust_cold_true_dev $TELOS build examples/todo/todo.telos --out $B/out-rust-c --target-dir $B/target-rust-c
run rust_cold_true_release $TELOS build examples/todo/todo.telos --release --out $B/out-rust-d --target-dir $B/target-rust-d
run llvm_runtime_cold_true_dev env CARGO_TARGET_DIR=$B/trt-c cargo build -q -p telos-rt --features runner
run llvm_runtime_cold_true_release env CARGO_TARGET_DIR=$B/trt-d cargo build -q --release -p telos-rt --features runner
unset SCCACHE_RECACHE
run llvm_program_cold_dev $TELOS build examples/todo/todo.telos --target llvm --out $B/out-llvm-c --runtime $B/trt-c/debug/libtelos_rt.a
run llvm_program_cold_release $TELOS build examples/todo/todo.telos --target llvm --release --out $B/out-llvm-d --runtime $B/trt-d/release/libtelos_rt.a
run llvm_program_warm_release $TELOS build examples/todo/todo.telos --target llvm --release --out $B/out-llvm-d --runtime $B/trt-d/release/libtelos_rt.a

echo "crates compiled in rust_cold_true_dev:"; grep 'Compiling' $B/rust_cold_true_dev.out | awk '{print $2}' | tr '\n' ' '; echo
sccache --show-stats 2>/dev/null | grep -E 'Compile requests|Cache hits|Cache misses|Forced recaches' | head -5
echo "sizes:"
for f in $B/target-rust-c/debug/todod $B/target-rust-d/release/todod $B/out-llvm-c/bin/todod $B/out-llvm-d/bin/todod $B/trt-c/debug/libtelos_rt.a $B/trt-d/release/libtelos_rt.a; do [ -f "$f" ] && echo "$(stat -c %s "$f") $f"; done
echo "ll files:"; find $B/out-llvm-c -name '*.ll' | head -5 | while read f; do echo "$(wc -l < "$f") $f"; done
echo EXIT=0
