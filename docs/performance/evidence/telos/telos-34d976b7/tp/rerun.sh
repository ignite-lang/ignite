#!/usr/bin/env bash
# usage: rerun.sh CELLS_FILE RESULTS_FILE   each line of CELLS_FILE: "quickack|stock BUILD WORKLOAD CONNECTIONS"
# Three runs per cell; before each run waits (up to 2 min) for the machine's runnable count to drop to nproc/2 or below.
set -u
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
K=$S/target-k
L=$K/release/telos-load
cells=$1
out=$2
half=$(( $(nproc) / 2 ))
bin_of() {
  case $1 in
    rust-release) echo $K/srv/rust-target/release/terminald ;;
    rust-debug) echo $K/srv/rust-target/debug/terminald ;;
    llvm-release) echo $K/srv/llvm-release/bin/terminald ;;
    llvm-debug) echo $K/srv/llvm-debug/bin/terminald ;;
  esac
}
runnable() {
  local total=0
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    r=$(awk '{ split($4, a, "/"); print a[1] - 1 }' /proc/loadavg)
    total=$((total + r))
    timeout 0.1 tail -f /dev/null
  done
  echo $((total / 10))
}
port=2700
for rep in 1 2 3; do
  while read -r mode build workload conns; do
    [ -z "$mode" ] && continue
    for _ in $(seq 1 8); do
      [ "$(runnable)" -le $half ] && break
      timeout 15 tail -f /dev/null
    done
    port=$((port+1)); if [ $port -gt 2990 ]; then port=2701; fi
    flag=""; if [ $mode = quickack ]; then flag=--quickack; fi
    timeout 120 $L $flag --server "$(bin_of $build)" --port $port --workload $workload --connections $conns --label "$build" --out "$out" > /dev/null 2>> "$out.err" || echo "FAILED rep=$rep $mode $build $workload $conns" >> "$out.err"
  done < $cells
done
echo EXIT=0
