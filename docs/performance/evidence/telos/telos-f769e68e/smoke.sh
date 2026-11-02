#!/bin/zsh
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
cd /home/timo/projects/telos || exit 2

wait_port() {
  log=$1
  for i in $(seq 1 100); do
    if grep -q 'listening on' $log; then
      sed -n 's/.*listening on 127.0.0.1:\([0-9]*\).*/\1/p' $log | head -1
      return 0
    fi
    timeout 0.2 tail -f /dev/null
  done
  return 1
}

terminal() {
  label=$1
  server=$2
  client=$3
  log=$S/logs/smoke-$label-terminald.log
  : > $log
  $server --port 0 > $log 2>&1 &
  pid=$!
  port=$(wait_port $log) || { echo "$label: terminald printed no listening line"; kill $pid; return 1; }
  out=$(printf 'echo hello\nquit\n' | timeout 10 $client --port $port 2>&1)
  code=$?
  kill $pid
  wait $pid 2>/dev/null
  echo "$label terminald port=$port pid=$pid; tn exit=$code output: ${out//$'\n'/ | }"
}

todo() {
  label=$1
  server=$2
  log=$S/logs/smoke-$label-todod.log
  : > $log
  $server --port 0 > $log 2>&1 &
  pid=$!
  port=$(wait_port $log) || { echo "$label: todod printed no listening line"; kill $pid; return 1; }
  out=$(curl -s -m 5 -i -X POST -H 'Content-Type: application/json' -d '{"title":"smoke"}' http://127.0.0.1:$port/todos | head -1)
  out2=$(curl -s -m 5 http://127.0.0.1:$port/todos)
  kill $pid
  wait $pid 2>/dev/null
  echo "$label todod port=$port pid=$pid; POST: $out; GET: $out2"
}

terminal rust target/debug/terminald target/debug/tn
terminal llvm target/ignite-llvm/terminal/bin/terminald target/ignite-llvm/terminal/bin/tn
todo rust target/debug/todod
todo llvm target/ignite-llvm/todo/bin/todod
echo "EXIT=0"
