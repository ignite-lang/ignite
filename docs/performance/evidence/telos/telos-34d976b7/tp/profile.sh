#!/usr/bin/env bash
# usage: profile.sh BUILD PORT   (BUILD: rust-release | llvm-release)
# perf record -g, perf stat and strace -c of terminald under pipe 16 and rr 16 (TCP_QUICKACK client).
set -u
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
K=$S/target-k
L=$K/release/telos-load
P=$S/tp/prof
mkdir -p $P
build=$1
port=$2
case $build in
  rust-release) bin=$K/srv/rust-target/release/terminald ;;
  llvm-release) bin=$K/srv/llvm-release/bin/terminald ;;
esac

start_server() {
  $bin --port $port > /dev/null 2> $P/$build-$1.server.log &
  spid=$!
  for _ in $(seq 1 100); do grep -q "listening on" $P/$build-$1.server.log && break; timeout 0.05 tail -f /dev/null; done
}
stop_server() {
  kill $spid 2> /dev/null
  wait $spid 2> /dev/null
}
cpu_split() {
  awk '{ s = $0; sub(/.*\) /, "", s); split(s, f, " "); print f[12], f[13] }' /proc/$1/stat
}

for workload in pipe rr; do
  # perf record
  start_server rec-$workload
  perf record -e cycles:u -g -p $spid -o $P/$build-$workload.perf.data -- sleep 7 > $P/$build-$workload.perf.log 2>&1 &
  ppid=$!
  $L --quickack --pid $spid --port $port --workload $workload --connections 16 --duration 5 --warmup 1 --label $build-perf > $P/$build-$workload.perf.json
  wait $ppid
  stop_server

  # perf stat: user-space instructions and cycles; utime/stime split
  start_server stat-$workload
  before=$(cpu_split $spid)
  perf stat -e instructions:u,cycles:u -p $spid -x, -o $P/$build-$workload.stat.csv -- sleep 6.5 > /dev/null 2>&1 &
  ppid=$!
  $L --quickack --pid $spid --port $port --workload $workload --connections 16 --duration 5 --warmup 0 --label $build-stat > $P/$build-$workload.stat.json
  wait $ppid
  after=$(cpu_split $spid)
  echo "$before $after" > $P/$build-$workload.cpusplit
  stop_server

  # strace -c
  start_server strace-$workload
  strace -c -f -p $spid -o $P/$build-$workload.strace.txt &
  tpid=$!
  timeout 0.5 tail -f /dev/null
  $L --quickack --pid $spid --port $port --workload $workload --connections 16 --duration 5 --warmup 0 --label $build-strace > $P/$build-$workload.strace.json
  kill -INT $tpid
  wait $tpid
  stop_server
done
echo EXIT=0
