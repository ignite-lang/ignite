#!/bin/zsh
# P2 slice 12 chain 5: wait until the shared tree builds again (peers' WIP in engine.rs and
# std/), then run P2's run/* unit tests, clippy, the inline builtins fixture on both targets
# and the todod pairs. Every step prints STEP <name> exit=<code>; ends with EXIT=<code>.
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

tries=0
while true; do
  tries=$((tries + 1))
  free=$(df --output=avail -BG / | tail -1 | tr -dc 0-9)
  if [ $free -lt 20 ]; then echo "disk ${free}G free, stop"; echo "EXIT=7"; exit 7; fi
  cargo build -p ignite 2>&1 | quiet > $S/s12-p2-chain5-build.txt
  bcode=${pipestatus[1]}
  fcode=9
  if [ $bcode -eq 0 ]; then
    $T/debug/ignite test $S/s12-p2-builtins.ignite --out $T/ignite-test --target-dir $T/ignite-test/cargo > $S/s12-p2-chain5-fixture.txt 2>&1
    fcode=$?
  fi
  echo "WAIT try=$tries $(date +%T) build=$bcode fixture=$fcode"
  if [ $bcode -eq 0 ] && [ $fcode -eq 0 ]; then break; fi
  if [ $tries -ge 30 ]; then
    echo "== last build output"; tail -30 $S/s12-p2-chain5-build.txt
    echo "== last fixture output"; tail -10 $S/s12-p2-chain5-fixture.txt
    echo "EXIT=8"; exit 8
  fi
  timeout 90 tail -f /dev/null
done
df -h / | tail -1

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
$T/debug/ignite test $S/s12-p2-builtins.ignite --out $T/ignite-test --target-dir $T/ignite-test/cargo 2>&1 | tail -8
step fixture-rust ${pipestatus[1]}
echo "== inline fixture, llvm"
$T/debug/ignite test $S/s12-p2-builtins.ignite --target llvm --out $T/ignite-test --runtime $T/ignite-test/rt/debug/libignite_rt.a 2>&1 | tail -8
step fixture-llvm ${pipestatus[1]}

echo "== runtimes for todod"
cargo build -q -p ignite-rt 2>&1 | quiet
cargo build -q -p ignite-rt --release 2>&1 | quiet
echo "== todod pairs after unit 1"
zsh $S/s12-p2-todod.sh unit1 3
step todod $?
echo "EXIT=0"
