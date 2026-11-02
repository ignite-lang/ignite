#!/bin/zsh
# P2 slice 12 chain 4: fmt on P2's files, run/* unit tests, the inline builtins fixture on both
# targets. Every step prints STEP <name> exit=<code>; ends with EXIT=<code>.
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
rustfmt --edition 2024 --check crates/ignite/src/run/{process,probe,clock,journal,control}.rs crates/ignite-io/src/*.rs crates/ignite-rt/src/*.rs 2>&1 | head -40
step fmt ${pipestatus[1]}

echo "== unit tests of crates/ignite/src/run"
cargo test -p ignite --lib run:: 2>&1 | quiet
step run-tests ${pipestatus[1]}

echo "== build ignite"
cargo build -p ignite 2>&1 | quiet
step build ${pipestatus[1]}

echo "== inline fixture, rust"
$T/debug/ignite test $S/s12-p2-builtins.ignite --out $T/ignite-test --target-dir $T/ignite-test/cargo 2>&1 | tail -20
step fixture-rust ${pipestatus[1]}
echo "== inline fixture, llvm"
$T/debug/ignite test $S/s12-p2-builtins.ignite --target llvm --out $T/ignite-test --runtime $T/ignite-test/rt/debug/libignite_rt.a 2>&1 | tail -20
step fixture-llvm ${pipestatus[1]}
echo "EXIT=0"
