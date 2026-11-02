#!/usr/bin/env bash
# usage: csw.sh BUILD PORT WORKLOAD   context switches per reply, summed over all server threads (sampled every 0.25 s, last value per thread)
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
log=$P/$build-$workload.csw-server.log
samples=$P/$build-$workload.csw.samples
: > $samples
$bin --port $port > /dev/null 2> $log &
spid=$!
for _ in $(seq 1 200); do grep -q "listening on" $log && break; timeout 0.05 tail -f /dev/null; done
(
  while kill -0 $spid 2> /dev/null; do
    for f in /proc/$spid/task/*/status; do
      awk -v t="$f" '/^voluntary_ctxt_switches/ {v=$2} /^nonvoluntary_ctxt_switches/ {n=$2} END {print t, v, n}' "$f" 2> /dev/null
    done >> $samples
    timeout 0.25 tail -f /dev/null
  done
) &
mpid=$!
$L --quickack --pid $spid --port $port --workload $workload --connections 16 --duration 5 --warmup 0 --label $build-csw > $P/$build-$workload.csw.json
kill $spid
wait $spid 2> /dev/null
wait $mpid 2> /dev/null
replies=$(grep -o '"replies":[0-9]*' $P/$build-$workload.csw.json | cut -d: -f2)
awk -v r=$replies '{ v[$1] = $2; n[$1] = $3 } END { for (t in v) { sv += v[t]; sn += n[t] }; printf "replies %d voluntary %d nonvoluntary %d per1000 voluntary %.1f nonvoluntary %.1f\n", r, sv, sn, sv * 1000 / r, sn * 1000 / r }' $samples
