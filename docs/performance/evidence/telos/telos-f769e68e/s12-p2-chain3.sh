#!/bin/zsh
# P2 slice 12 chain 3: build the ignite CLI from the current tree, run P2's run/* unit tests,
# clippy on P2's crates, the inline builtins fixture on both targets, then the todod pairs.
# Every step prints STEP <name> exit=<code>; a failed build stops the chain. Ends with EXIT=<code>.
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

echo "== build ignite (all targets)"
cargo build -p ignite --all-targets 2>&1 | quiet
code=${pipestatus[1]}
step build $code
if [ $code -ne 0 ]; then echo "EXIT=$code"; exit $code; fi

echo "== unit tests of crates/ignite/src/run"
cargo test -p ignite --lib run:: 2>&1 | quiet
step run-tests ${pipestatus[1]}

echo "== clippy"
cargo clippy -p ignite-io -p ignite-rt -p ignite-rustgen -p ignite-llvm -p ignite --all-targets -- -D warnings 2>&1 | quiet | head -60
step clippy ${pipestatus[1]}

echo "== runner runtime"
cargo build -q -p ignite-rt --features runner --target-dir $T/ignite-test/rt 2>&1 | quiet
step runner ${pipestatus[1]}

echo "== inline fixture, rust"
$T/debug/ignite test $S/s12-p2-builtins.ignite --out $T/ignite-test --target-dir $T/ignite-test/cargo 2>&1 | tail -20
step fixture-rust ${pipestatus[1]}
echo "== inline fixture, llvm"
$T/debug/ignite test $S/s12-p2-builtins.ignite --target llvm --out $T/ignite-test --runtime $T/ignite-test/rt/debug/libignite_rt.a 2>&1 | tail -20
step fixture-llvm ${pipestatus[1]}

echo "== runtimes for todod"
cargo build -q -p ignite-rt 2>&1 | quiet
cargo build -q -p ignite-rt --release 2>&1 | quiet
echo "== todod pairs after unit 1"
zsh $S/s12-p2-todod.sh unit1 3
step todod $?
echo "EXIT=0"
