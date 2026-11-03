#!/usr/bin/env bash
# usage: cg.sh BUILD PORT   callgrind of terminald under rr 1 for 3 s
set -u
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
K=$S/target-k
L=$K/release/telos-load
P=$S/tp/prof
build=$1
port=$2
case $build in
  rust-release) bin=$K/srv/rust-target/release/terminald ;;
  llvm-release) bin=$K/srv/llvm-release/bin/terminald ;;
esac
log=$P/$build.cg-server.log
valgrind --tool=callgrind --callgrind-out-file=$P/$build.callgrind.out $bin --port $port > /dev/null 2> $log &
vpid=$!
for _ in $(seq 1 400); do grep -q "listening on" $log && break; timeout 0.05 tail -f /dev/null; done
$L --quickack --port $port --workload rr --connections 1 --duration 3 --warmup 0 --label $build-cg > $P/$build.cg.json
kill -INT $vpid
wait $vpid
echo EXIT=0
