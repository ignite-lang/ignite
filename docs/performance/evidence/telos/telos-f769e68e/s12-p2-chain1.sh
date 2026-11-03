#!/bin/zsh
# P2 slice 12 chain 1: tests of the four crates P2 touched, the runner runtime, the inline
# builtins fixture on both targets, then the baseline tree's compiler and runtimes for the
# invariant-9 measurement. Every step prints STEP <name> exit=<code>; the log ends with EXIT=0.
set -o pipefail
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
T=$S/target-s12-p2
B=$S/target-s12-before-p2
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
quiet() { grep -v '^\s*Compiling\|^\s*Blocking\|^\s*Locking\|^\s*Finished\|^test .* ok$\|^\s*$\|^running \|Doc-tests' }
step() { echo "STEP $1 exit=$2"; }
cd /home/timo/projects/telos || exit 9
df -h / | tail -1

echo "== tests: ignite-io ignite-rt ignite-rustgen ignite-llvm"
CARGO_TARGET_DIR=$T cargo test -p ignite-io -p ignite-rt -p ignite-rustgen -p ignite-llvm 2>&1 | quiet
step tests $?

echo "== runner runtime"
CARGO_TARGET_DIR=$T cargo build -q -p ignite-rt --features runner --target-dir $T/ignite-test/rt 2>&1 | quiet
step runner $?

echo "== inline fixture, rust"
$T/debug/ignite test $S/s12-p2-builtins.ignite --out $T/ignite-test --target-dir $T/ignite-test/cargo 2>&1 | tail -20
step fixture-rust $?
echo "== inline fixture, llvm"
$T/debug/ignite test $S/s12-p2-builtins.ignite --target llvm --out $T/ignite-test --runtime $T/ignite-test/rt/debug/libignite_rt.a 2>&1 | tail -20
step fixture-llvm $?

echo "== after-tree runtimes for todod"
CARGO_TARGET_DIR=$T cargo build -q -p ignite-rt 2>&1 | quiet
step rt-dev $?
CARGO_TARGET_DIR=$T cargo build -q -p ignite-rt --release 2>&1 | quiet
step rt-release $?

echo "== baseline tree compiler and runtimes"
df -h / | tail -1
cd $S/s12-before || exit 9
CARGO_TARGET_DIR=$B cargo build -q -p ignite 2>&1 | quiet
step base-ignite $?
CARGO_TARGET_DIR=$B cargo build -q -p ignite-rt 2>&1 | quiet
step base-rt-dev $?
CARGO_TARGET_DIR=$B cargo build -q -p ignite-rt --release 2>&1 | quiet
step base-rt-release $?
df -h / | tail -1
echo "EXIT=0"
