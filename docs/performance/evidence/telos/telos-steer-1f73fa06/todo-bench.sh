#!/bin/zsh
ulimit -c 0
B=/home/timo/.cache/scratch/1f73fa06-e945-4828-ae60-02b7cdacb054/todo-bench
REPO=/home/timo/projects/telos
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
rm -rf "$B"; mkdir -p "$B"
rsync -a --exclude target --exclude .idea --exclude .git "$REPO/" "$B/tree/"
git -C "$REPO" archive 44334b0 std examples | tar -x -C "$B/tree"
cd "$B/tree" || exit 1
echo "nproc=$(nproc) rustc=$(rustc --version) clang=$(clang --version | head -1) wrapper=$(cargo config get build.rustc-wrapper 2>/dev/null | tail -1)"

t0=$(date +%s.%N)
CARGO_TARGET_DIR=$B/tc cargo build -q -p telos 2>&1 | tail -5
echo "compiler_build secs=$(printf '%.1f' $(( $(date +%s.%N) - t0 ))) status=${pipestatus[1]}"
TELOS=$B/tc/debug/telos
$TELOS check examples/todo/todo.telos 2>&1 | tail -3; echo "check status=${pipestatus[1]}"

run() {
  local label=$1; shift
  local t0=$(date +%s.%N)
  "$@" 2>&1 | tail -3
  echo "$label secs=$(printf '%.1f' $(( $(date +%s.%N) - t0 ))) status=${pipestatus[1]}"
}

run rust_cold_sccache $TELOS build examples/todo/todo.telos --out $B/out-rust-a --target-dir $B/target-rust-a
run rust_warm_sccache $TELOS build examples/todo/todo.telos --out $B/out-rust-a --target-dir $B/target-rust-a
RUSTC_WRAPPER="" run rust_cold_nowrapper $TELOS build examples/todo/todo.telos --out $B/out-rust-b --target-dir $B/target-rust-b
RUSTC_WRAPPER="" run rust_warm_nowrapper $TELOS build examples/todo/todo.telos --out $B/out-rust-b --target-dir $B/target-rust-b

run llvm_runtime_cold_sccache env CARGO_TARGET_DIR=$B/trt-a cargo build -q -p telos-rt --features runner
RT_A=$B/trt-a/debug/libtelos_rt.a
run llvm_cold_program $TELOS build examples/todo/todo.telos --target llvm --out $B/out-llvm-a --runtime $RT_A
run llvm_warm_program $TELOS build examples/todo/todo.telos --target llvm --out $B/out-llvm-a --runtime $RT_A
RUSTC_WRAPPER="" run llvm_runtime_cold_nowrapper env CARGO_TARGET_DIR=$B/trt-b cargo build -q -p telos-rt --features runner

echo "sizes:"; ls -la $B/out-rust-a/target/debug/todod $B/out-rust-a/target/debug/* 2>/dev/null | grep -v '\.d$' | awk '{print $5, $9}' | head -4
ls -la $B/out-llvm-a/bin/* 2>/dev/null | awk '{print $5, $9}' | head -4
ls -la $RT_A | awk '{print $5, $9}'
echo "ll_lines=$(cat $B/out-llvm-a/*.ll 2>/dev/null | wc -l) rust_gen_lines=$(cat $B/out-rust-a/src/*.rs 2>/dev/null | wc -l)"
echo EXIT=0
