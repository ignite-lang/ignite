#!/usr/bin/env bash
# usage: ctx.sh BUILD PORT WORKLOAD   perf stat of software events (context switches, migrations) and utime/stime
set -u
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
K=$S/target-k
L=$K/release/telos-load
P=$S/tp/prof
build=$1
port=$2
workload=$3
case $build in
  rust-release) bin=$K/srv/rust-target/release/terminald ;;
  llvm-release) bin=$K/srv/llvm-release/bin/terminald ;;
esac
log=$P/$build-$workload.ctx-server.log
$bin --port $port > /dev/null 2> $log &
spid=$!
for _ in $(seq 1 200); do grep -q "listening on" $log && break; timeout 0.05 tail -f /dev/null; done
perf stat -e context-switches,cpu-migrations,task-clock -p $spid -x, -o $P/$build-$workload.ctx.csv -- sleep 6 > /dev/null 2>&1 &
ppid=$!
$L --quickack --pid $spid --port $port --workload $workload --connections 16 --duration 5 --warmup 0 --label $build-ctx > $P/$build-$workload.ctx.json
wait $ppid
awk '{ s = $0; sub(/.*\) /, "", s); split(s, f, " "); print "utime_ticks", f[12], "stime_ticks", f[13] }' /proc/$spid/stat >> $P/$build-$workload.ctx.csv
kill $spid
wait $spid 2> /dev/null
echo EXIT=0
