#!/usr/bin/env bash
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
K=$S/target-k
L=$K/release/telos-load
port=2440
for build in rust-release llvm-release; do
  case $build in
    rust-release) bin=$K/srv/rust-target/release/terminald ;;
    llvm-release) bin=$K/srv/llvm-release/bin/terminald ;;
  esac
  for spec in "rr 1" "rr 16" "pipe 16" "pipe 256"; do
    set -- $spec
    port=$((port+1))
    timeout 60 $L --quickack --server $bin --port $port --workload $1 --connections $2 --duration 2 --warmup 0.5 --label $build
  done
done
echo EXIT=0
