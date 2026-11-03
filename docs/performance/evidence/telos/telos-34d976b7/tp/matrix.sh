#!/usr/bin/env bash
# usage: matrix.sh RESULTS_FILE REPS [build...]
set -u
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
K=$S/target-k
L=$K/release/telos-load
out=$1
reps=$2
shift 2
builds=("$@")
if [ ${#builds[@]} -eq 0 ]; then builds=(rust-release llvm-release rust-debug llvm-debug); fi
bin_of() {
  case $1 in
    rust-release) echo $K/srv/rust-target/release/terminald ;;
    rust-debug) echo $K/srv/rust-target/debug/terminald ;;
    llvm-release) echo $K/srv/llvm-release/bin/terminald ;;
    llvm-debug) echo $K/srv/llvm-debug/bin/terminald ;;
  esac
}
port=2450
cells=("rr 1" "rr 16" "rr 256" "pipe 1" "pipe 16" "pipe 256" "connect 1" "connect 16")
for rep in $(seq 1 $reps); do
  for mode in quickack stock; do
    for cell in "${cells[@]}"; do
      set -- $cell
      for build in "${builds[@]}"; do
        port=$((port+1)); if [ $port -gt 2990 ]; then port=2451; fi
        flag=""; if [ $mode = quickack ]; then flag=--quickack; fi
        timeout 120 $L $flag --server "$(bin_of $build)" --port $port --workload $1 --connections $2 --label "$build" --out "$out" > /dev/null 2>> "$out.err" || echo "FAILED rep=$rep $mode $build $cell" >> "$out.err"
      done
    done
  done
done
echo EXIT=0
