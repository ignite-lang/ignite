#!/usr/bin/env bash
# usage: strace.sh BUILD PORT   strace -c -f of terminald started under strace, pipe 16 and rr 16 (TCP_QUICKACK client)
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
for workload in pipe rr; do
  log=$P/$build-$workload.futex-server.log
  strace -c -f --seccomp-bpf -e trace=futex,epoll_wait,write -o $P/$build-$workload.futex.txt $bin --port $port > /dev/null 2> $log &
  tpid=$!
  for _ in $(seq 1 200); do grep -q "listening on" $log && break; timeout 0.05 tail -f /dev/null; done
  spid=$(cat /proc/$tpid/task/$tpid/children | awk '{print $1}')
  $L --quickack --pid $spid --port $port --workload $workload --connections 16 --duration 5 --warmup 0 --label $build-strace > $P/$build-$workload.futex.json
  kill $spid
  wait $tpid
done
echo EXIT=0
