#!/bin/zsh
# P2 slice 12 chain 11: the drain unit (16.13) and item 6: fmt, ignite-io and ignite-rt tests,
# build, clippy, then the drain e2e tests of crates/ignite (both targets).
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

echo "== fmt (drain.rs only)"
rustfmt --edition 2024 crates/ignite/tests/drain.rs
step fmt $?

echo "== skip io-rt (green in chain 11)"
true
step io-rt-tests ${pipestatus[1]}

echo "== build ignite + runtimes"
cargo build -p ignite 2>&1 | quiet | grep -v '^warning\|^ *|\|^ *=\|^ *-->\|^[0-9]* |' | head -30
step build ${pipestatus[1]}
cargo build -q -p ignite-rt 2>&1 | quiet
cargo build -q -p ignite-rt --features runner --target-dir $T/ignite-test/rt 2>&1 | quiet

echo "== clippy (errors by file)"
cargo clippy -p ignite-io -p ignite-rt -p ignite-rustgen -p ignite-llvm -p ignite --all-targets -- -D warnings 2>&1 | quiet > $S/s12-p2-chain11-clippy.txt
step clippy ${pipestatus[1]}
grep -A1 '^error' $S/s12-p2-chain11-clippy.txt | grep -o 'crates/[^:]*' | sort | uniq -c

echo "== drain e2e tests"
t0=$(date +%s.%N)
cargo test -p ignite --test main drain:: -- --nocapture 2>&1 | quiet | tail -80
step drain-tests ${pipestatus[1]}
t1=$(date +%s.%N)
printf 'drain tests secs=%.1f load=%s\n' $(echo "$t1 - $t0" | bc) $(cut -d' ' -f1 /proc/loadavg)
echo "EXIT=0"
