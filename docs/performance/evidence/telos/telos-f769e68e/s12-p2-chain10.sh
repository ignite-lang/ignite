#!/bin/zsh
# P2 slice 12 chain 10: the gate items of the brief on the shared tree: cargo test of
# ignite-io and ignite-rt, cargo build --workspace --all-targets, task test-ignite (both
# targets). Every step prints STEP <name> exit=<code>; ends with EXIT=<code>.
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

echo "== cargo test -p ignite-io -p ignite-rt"
cargo test -p ignite-io -p ignite-rt 2>&1 | quiet
step io-rt-tests ${pipestatus[1]}

echo "== cargo build --workspace --all-targets"
cargo build --workspace --all-targets 2>&1 | quiet | grep -v '^warning\|^ *|\|^ *=\|^ *-->\|^[0-9]* |' | head -40
step workspace-build ${pipestatus[1]}

echo "== task test-ignite"
t0=$(date +%s.%N)
task test-ignite 2>&1 | quiet | grep -v '^task: \[' | tail -30
code=${pipestatus[1]}
t1=$(date +%s.%N)
step test-ignite $code
printf 'test-ignite secs=%.1f load=%s\n' $(echo "$t1 - $t0" | bc) $(cut -d' ' -f1 /proc/loadavg)
echo "EXIT=0"
