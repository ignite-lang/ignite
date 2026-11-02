#!/usr/bin/env bash
set -u
B=/home/timo/projects/telos/target/telos-llvm/terminal/bin
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
PORT=23917
"$B/terminald" --port "$PORT" > "$S/llvm_server.log" 2>&1 &
SERVER=$!
for _ in $(seq 50); do
  if (exec 3<>/dev/tcp/127.0.0.1/$PORT) 2>/dev/null; then break; fi
  timeout 0.1 tail -f /dev/null
done
printf 'help\necho hello llvm\nstatus\nbogus\nquit\n' | timeout 10 "$B/tn" --port "$PORT" > "$S/llvm_client.out" 2>&1
echo "tn exit=$?"
cat "$S/llvm_client.out"
"$B/tn" --port 1 > /dev/null 2>&1 < /dev/null
echo "tn to closed port exit=$?"
"$B/terminald" --port nope > /dev/null 2>&1
echo "terminald bad port exit=$?"
kill "$SERVER"
wait "$SERVER" 2>/dev/null
echo "server log:"
cat "$S/llvm_server.log"
