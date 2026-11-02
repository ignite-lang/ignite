#!/bin/zsh
# P2 slice 12 chain 7: fmt and clippy on P2's crates (errors in other owners' files are listed,
# not fixed), the run/* unit tests, then the todod pairs cold through sccache
# (SCCACHE_RECACHE=1) with a size attribution of the generated std of both trees.
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
rustfmt --edition 2024 --check crates/ignite/src/run/{process,probe,clock,journal,control}.rs crates/ignite-io/src/*.rs crates/ignite-rt/src/*.rs crates/ignite-rustgen/src/lib.rs crates/ignite-llvm/src/lower.rs 2>&1 | head -40
step fmt ${pipestatus[1]}

echo "== clippy (errors by file)"
cargo clippy -p ignite-io -p ignite-rt -p ignite-rustgen -p ignite-llvm -p ignite --all-targets -- -D warnings 2>&1 | quiet > $S/s12-p2-chain7-clippy.txt
step clippy ${pipestatus[1]}
grep -A1 '^error' $S/s12-p2-chain7-clippy.txt | grep -o 'crates/[^:]*' | sort | uniq -c

echo "== unit tests of crates/ignite/src/run"
cargo test -p ignite --lib run:: 2>&1 | quiet
step run-tests ${pipestatus[1]}

echo "== runtimes for todod"
cargo build -q -p ignite-rt 2>&1 | quiet
cargo build -q -p ignite-rt --release 2>&1 | quiet
echo "== todod pairs after unit 1, cold through sccache"
zsh $S/s12-p2-todod.sh unit1cold 3
step todod $?

echo "== size attribution of the generated std (base vs new, pair 1)"
for which in base new; do
  d=$S/s12-p2-bench-unit1cold/$which-rust-dev-1/project
  echo "$which rust: $(find $d/std -name '*.rs' | xargs cat | wc -l) lines of generated std, $(find $d/src -name '*.rs' | xargs cat | wc -l) lines of program"
  l=$S/s12-p2-bench-unit1cold/$which-llvm-dev-1/project/llvm
  echo "$which llvm: $(wc -l < $l/std.ll) lines std.ll, $(grep -c '^define' $l/std.ll) defines; program.ll $(wc -l < $l/program.ll) lines"
done
echo "EXIT=0"
