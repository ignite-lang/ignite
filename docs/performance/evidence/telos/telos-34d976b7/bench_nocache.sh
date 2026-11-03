#!/usr/bin/env bash
set -u
cd /home/timo/projects/telos
export CARGO_PROFILE_DEV_DEBUG=line-tables-only CARGO_INCREMENTAL=0
export RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER=
W=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/bench-nc
mkdir -p "$W"
EX=examples/terminal/terminal.telos
T=./target/debug/telos

stamp() {
  local label=$1; shift
  local start end
  start=$(date +%s.%N)
  "$@" > "$W/$label.log" 2>&1
  local rc=$?
  end=$(date +%s.%N)
  printf '%s\t%s\texit=%s\n' "$label" "$(echo "$end - $start" | bc)" "$rc"
}

stamp rust-debug-cold   $T build $EX --out "$W/rust-debug"   --target-dir "$W/rust-debug-target"
stamp rust-release-cold $T build $EX --out "$W/rust-release" --target-dir "$W/rust-release-target" --release
stamp rt-debug-cold     env CARGO_TARGET_DIR="$W/rt-target" cargo build -p telos-rt
stamp rt-release-cold   env CARGO_TARGET_DIR="$W/rt-target" cargo build -p telos-rt --release
stamp llvm-debug        $T build $EX --target llvm --out "$W/llvm-debug"   --runtime "$W/rt-target/debug/libtelos_rt.a"
stamp llvm-release      $T build $EX --target llvm --out "$W/llvm-release" --runtime "$W/rt-target/release/libtelos_rt.a" --release
grep -h "Compiling" "$W"/rust-debug-cold.log | wc -l
grep -h "Compiling" "$W"/rt-debug-cold.log | wc -l
grep -c sccache "$W"/*.log
echo EXIT=0
