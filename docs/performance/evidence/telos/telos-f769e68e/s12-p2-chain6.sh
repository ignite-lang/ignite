#!/bin/zsh
# P2 slice 12 chain 6: the builtins fixture with `ignite test` and `ignite build` on both
# targets (prober without and with a --ref binding), P2's run/* unit tests, clippy, then the
# todod pairs. Every step prints STEP <name> exit=<code>; ends with EXIT=<code>.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
T=$S/target-s12-p2
F=$S/s12-p2-builtins.ignite
export CARGO_TARGET_DIR=$T
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
quiet() { grep -v '^\s*Compiling\|^\s*Blocking\|^\s*Locking\|^\s*Finished\|^\s*Checking\|^test .* ok$\|^\s*$\|^running \|Doc-tests\|^\s*Running'; true }
step() { echo "STEP $1 exit=$2"; }
cd /home/timo/projects/telos || exit 9
df -h / | tail -1

echo "== build ignite and the runner runtime"
cargo build -p ignite 2>&1 | quiet
step build ${pipestatus[1]}
cargo build -q -p ignite-rt --features runner --target-dir $T/ignite-test/rt 2>&1 | quiet
step runner ${pipestatus[1]}
cargo build -q -p ignite-rt 2>&1 | quiet
step runtime ${pipestatus[1]}

echo "== inline fixture, rust"
$T/debug/ignite test $F --out $T/ignite-test --target-dir $T/ignite-test/cargo 2>&1 | tail -8
step fixture-rust ${pipestatus[1]}
echo "== inline fixture, llvm"
$T/debug/ignite test $F --target llvm --out $T/ignite-test --runtime $T/ignite-test/rt/debug/libignite_rt.a 2>&1 | tail -8
step fixture-llvm ${pipestatus[1]}

prober() {
  local label=$1 bin=$2
  echo "-- $label: without binding"
  $bin > $S/s12-p2-prober-out.txt 2> $S/s12-p2-prober-err.txt
  local rc=$?
  echo "exit=$rc stdout=[$(cat $S/s12-p2-prober-out.txt)] stderr=[$(cat $S/s12-p2-prober-err.txt)]"
  [ $rc -eq 1 ] && grep -q '^error: no binding for Probes.server$' $S/s12-p2-prober-err.txt && [ ! -s $S/s12-p2-prober-out.txt ]
  step $label-unbound $?
  echo "-- $label: with binding"
  $bin --ref Probes.server=127.0.0.1:4242 > $S/s12-p2-prober-out.txt 2> $S/s12-p2-prober-err.txt
  rc=$?
  echo "exit=$rc stdout=[$(cat $S/s12-p2-prober-out.txt)] stderr=[$(cat $S/s12-p2-prober-err.txt)]"
  [ $rc -eq 0 ] && [ "$(cat $S/s12-p2-prober-out.txt)" = "127.0.0.1:4242" ]
  step $label-bound $?
  echo "-- $label: malformed binding"
  $bin --ref Probes.server > $S/s12-p2-prober-out.txt 2> $S/s12-p2-prober-err.txt
  rc=$?
  echo "exit=$rc stderr=[$(cat $S/s12-p2-prober-err.txt)]"
  [ $rc -eq 2 ]
  step $label-malformed $?
}

echo "== ignite build, rust"
rm -rf $S/s12-p2-prober-rust
$T/debug/ignite build $F --out $S/s12-p2-prober-rust/project --target-dir $S/s12-p2-prober-rust/target 2>&1 | quiet
step build-rust ${pipestatus[1]}
prober rust $S/s12-p2-prober-rust/target/debug/prober
echo "== ignite build, llvm"
rm -rf $S/s12-p2-prober-llvm
$T/debug/ignite build $F --target llvm --out $S/s12-p2-prober-llvm --runtime $T/debug/libignite_rt.a 2>&1 | quiet
step build-llvm ${pipestatus[1]}
prober llvm $S/s12-p2-prober-llvm/bin/prober

echo "== unit tests of crates/ignite/src/run"
cargo test -p ignite --lib run:: 2>&1 | quiet
step run-tests ${pipestatus[1]}

echo "== clippy"
cargo clippy -p ignite-io -p ignite-rt -p ignite-rustgen -p ignite-llvm -p ignite --all-targets -- -D warnings 2>&1 | quiet | head -60
step clippy ${pipestatus[1]}

echo "== release runtime for todod"
cargo build -q -p ignite-rt --release 2>&1 | quiet
echo "== todod pairs after unit 1"
zsh $S/s12-p2-todod.sh unit1 3
step todod $?
echo "EXIT=0"
