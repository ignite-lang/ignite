#!/usr/bin/env bash
# One dhat run of ember (Rust target, release) under the ember tournament's allocation workload.
#
#   dhat.sh TREE EMBER_SOURCE EMBERLOAD OUT [PORT]
#
# TREE is an ignite checkout with target/debug/ignite built; EMBER_SOURCE is ember.ignite of the
# examples repository; EMBERLOAD is a built emberload binary. The workload is the tournament's
# alloc phase (examples ember/tournament/measure.sh): `4 2000 10` (8,000 INCR), `2 1000 10 set`
# (2,000 SET of 1,000 bytes), then FLUSHDB. Writes OUT/dhat.json, OUT/server.log, OUT/total.txt
# and OUT/sites.tsv (blocks, bytes, first three program frames, allocating library frame).
# SKIP_BUILD=1 reuses OUT/build.
set -u
TREE=$1; EMBER=$2; LOADGEN=$3; OUT=$4; PORT=${5:-6421}
HERE=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$OUT"
if [ "${SKIP_BUILD:-0}" != 1 ]; then
  "$TREE/target/debug/ignite" build --target rust --release --std "$TREE/std" --out "$OUT/build" "$EMBER" || exit 10
fi
BIN=$OUT/build/target/release/emberd
[ -x "$BIN" ] || { echo "no binary $BIN"; exit 11; }
valgrind --tool=dhat --dhat-out-file="$OUT/dhat.json" "$BIN" --port "$PORT" > "$OUT/server.log" 2>&1 &
SERVER=$!
up=0
for _ in $(seq 600); do
  if (exec 3<> "/dev/tcp/127.0.0.1/$PORT") 2> /dev/null; then up=1; break; fi
  timeout 0.05 tail -f /dev/null
done
[ $up = 1 ] || { kill -KILL $SERVER; echo "server did not start"; exit 12; }
echo "$PORT 4 2000 10" | timeout 600 "$LOADGEN"
echo "$PORT 2 1000 10 set" | timeout 600 "$LOADGEN"
exec 3<> "/dev/tcp/127.0.0.1/$PORT"
printf 'FLUSHDB\r\nQUIT\r\n' >&3
cat <&3 > /dev/null
exec 3<&-
kill -TERM $SERVER
wait $SERVER
grep -E 'Total:|At t-gmax:|At t-end:' "$OUT/server.log" | sed 's/^==[0-9]*== *//' > "$OUT/total.txt"
cat "$OUT/total.txt"
jq -r -f "$HERE/sites.jq" "$OUT/dhat.json" > "$OUT/sites.tsv"
