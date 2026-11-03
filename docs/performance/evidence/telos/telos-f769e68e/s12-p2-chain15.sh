#!/bin/zsh
# P2 slice 12 chain 15: verify the TasksMax=2 cause at run time, then the coordinator's
# sequence: full `cargo test -p ignite --test main`, drain:: again, clippy -D warnings on the
# workspace, fmt --check. Every step prints STEP <name> exit=<code>; ends with EXIT=<code>.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
T=$S/target-s12-p2
export CARGO_TARGET_DIR=$T
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
quiet() { grep -v '^\s*Compiling\|^\s*Blocking\|^\s*Locking\|^\s*Finished\|^\s*Checking\|^test .* ok$\|^\s*$\|^running \|Doc-tests\|^\s*Running'; true }
step() { echo "STEP $1 exit=$2"; }
cd /home/timo/projects/telos || exit 9
df -h / | tail -1

echo "== runtime check: terminald under TasksMax=2, first connection"
bin=$T/tmp/run-cargo/debug/terminald
systemd-run --user --scope --quiet -p TasksMax=2 $bin --port 0 > $S/s12-p2-tasksmax.out 2> $S/s12-p2-tasksmax.err &
helper=$!
for i in $(seq 1 50); do grep -q 'listening on' $S/s12-p2-tasksmax.err && break; timeout 0.1 tail -f /dev/null; done
port=$(grep -o 'listening on 127.0.0.1:[0-9]*' $S/s12-p2-tasksmax.err | cut -d: -f2)
echo "port=$port"
pid=$(for p in /proc/[0-9]*; do [ "$(readlink $p/exe 2>/dev/null)" = "$bin" ] && echo ${p#/proc/}; done | head -1)
echo "pid=$pid threads: $(grep Threads /proc/$pid/status 2>/dev/null)"
python3 -I $S/s12-p2-connect.py $port
timeout 1 tail -f /dev/null
echo "stderr:"; cat $S/s12-p2-tasksmax.err
kill -9 $pid 2>/dev/null; wait $helper 2>/dev/null
step tasksmax-check 0

echo "== full cargo test -p ignite --test main"
t0=$(date +%s.%N)
cargo test -p ignite --test main > $S/s12-p2-chain15-main.txt 2>&1
code=$?
t1=$(date +%s.%N)
grep '^test result\|FAILED\|panicked at' $S/s12-p2-chain15-main.txt | head -20
step main-tests $code
printf 'main tests secs=%.1f load=%s\n' $(echo "$t1 - $t0" | bc) $(cut -d' ' -f1 /proc/loadavg)

echo "== drain:: again"
cargo test -p ignite --test main drain:: 2>&1 | grep '^test \|^test result'
step drain-tests ${pipestatus[1]}

echo "== clippy workspace"
cargo clippy --workspace --all-targets -- -D warnings 2>&1 | quiet | head -30
step clippy ${pipestatus[1]}

echo "== fmt"
cargo fmt --check 2>&1 | head -20
step fmt ${pipestatus[1]}
echo "EXIT=0"
