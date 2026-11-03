#!/bin/zsh
# P2 slice 12 chain 13: the kill guard fix, run/* unit tests, the drain and item-6 e2e tests
# (full output in s12-p2-chain13-e2e.txt), then the gate items and the todod pairs after unit 5.
# Every step prints STEP <name> exit=<code>; ends with EXIT=<code>.
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

echo "== fmt"
rustfmt --edition 2024 --check crates/ignite/src/run/process.rs crates/ignite/tests/drain.rs crates/ignite-io/src/*.rs crates/ignite-rt/src/*.rs 2>&1 | head -20
step fmt ${pipestatus[1]}

echo "== run/* unit tests"
cargo test -p ignite --lib run:: 2>&1 | quiet
step run-tests ${pipestatus[1]}

echo "== drain e2e tests (3), full output in s12-p2-chain13-e2e.txt"
t0=$(date +%s.%N)
cargo test -p ignite --test main drain:: > $S/s12-p2-chain13-e2e.txt 2>&1
code=$?
t1=$(date +%s.%N)
grep '^test drain::\|^test result' $S/s12-p2-chain13-e2e.txt
step drain-tests $code
printf 'drain tests secs=%.1f load=%s\n' $(echo "$t1 - $t0" | bc) $(cut -d' ' -f1 /proc/loadavg)
if [ $code -ne 0 ]; then grep -n 'panicked' -A3 $S/s12-p2-chain13-e2e.txt | cut -c1-700 | head -30; fi

echo "== clippy (errors by file)"
cargo clippy -p ignite-io -p ignite-rt -p ignite-rustgen -p ignite-llvm -p ignite --all-targets -- -D warnings 2>&1 | quiet > $S/s12-p2-chain13-clippy.txt
step clippy ${pipestatus[1]}
grep -A1 '^error' $S/s12-p2-chain13-clippy.txt | grep -o 'crates/[^:]*' | sort | uniq -c

echo "== cargo test -p ignite-io -p ignite-rt"
cargo test -p ignite-io -p ignite-rt 2>&1 | quiet | grep 'test result\|FAILED\|panicked'
step io-rt-tests ${pipestatus[1]}

echo "== cargo build --workspace --all-targets"
cargo build --workspace --all-targets 2>&1 | quiet | grep -v '^warning\|^ *|\|^ *=\|^ *-->\|^[0-9]* |' | head -20
step workspace-build ${pipestatus[1]}

echo "== task test-ignite"
task test-ignite 2>&1 | quiet | grep 'test result\|FAILED\|error' | head
step test-ignite ${pipestatus[1]}

echo "== runtimes for todod"
cargo build -q -p ignite-rt 2>&1 | quiet
cargo build -q -p ignite-rt --release 2>&1 | quiet
echo "== todod pairs after unit 5, cold through sccache"
zsh $S/s12-p2-todod.sh unit5cold 3
step todod $?
echo "EXIT=0"
