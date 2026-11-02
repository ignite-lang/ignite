#!/bin/zsh
# P2 slice 12 chain 9: the two main-generation fixes (rustgen bare-name call through the
# expected type, LLVM omitted Signal field), the rustgen and llvm tests (goldens must stay
# byte-identical), `ignite build` of the design file on both targets, a smoke run of its
# promptProbe without a binding, clippy on P2's crates. Every step prints STEP <name> exit=<code>.
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
rustfmt --edition 2024 --check crates/ignite-rustgen/src/lib.rs crates/ignite-llvm/src/lib.rs crates/ignite-llvm/src/lower.rs 2>&1 | head -30
step fmt ${pipestatus[1]}

echo "== build ignite and the runtime"
cargo build -p ignite 2>&1 | quiet
step build ${pipestatus[1]}
cargo build -q -p ignite-rt 2>&1 | quiet
step runtime ${pipestatus[1]}

echo "== rustgen and llvm tests"
cargo test -p ignite-rustgen -p ignite-llvm 2>&1 | quiet
step gen-tests ${pipestatus[1]}
echo "golden mtimes newer than this script's start:"; find crates/ignite-rustgen/tests/golden crates/ignite-llvm/tests -newer $S/s12-p2-chain9.sh -type f | head

echo "== design file, rust"
rm -rf $S/s12-p2-design-rust
$T/debug/ignite build examples/terminal/terminal.design.ignite --out $S/s12-p2-design-rust/project --target-dir $S/s12-p2-design-rust/target 2>&1 | quiet | tail -12
step design-rust ${pipestatus[1]}
echo "-- promptProbe without binding (rust)"
$S/s12-p2-design-rust/target/debug/promptProbe; echo "exit=$?"

echo "== design file, llvm"
rm -rf $S/s12-p2-design-llvm
$T/debug/ignite build examples/terminal/terminal.design.ignite --target llvm --out $S/s12-p2-design-llvm --runtime $T/debug/libignite_rt.a 2>&1 | tail -12
step design-llvm ${pipestatus[1]}
echo "-- promptProbe without binding (llvm)"
$S/s12-p2-design-llvm/bin/promptProbe; echo "exit=$?"

echo "== clippy (errors by file)"
cargo clippy -p ignite-io -p ignite-rt -p ignite-rustgen -p ignite-llvm -p ignite --all-targets -- -D warnings 2>&1 | quiet > $S/s12-p2-chain9-clippy.txt
step clippy ${pipestatus[1]}
grep -A1 '^error' $S/s12-p2-chain9-clippy.txt | grep -o 'crates/[^:]*' | sort | uniq -c
echo "EXIT=0"
