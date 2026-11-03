#!/bin/zsh
# P2 slice 12 chain 2: the inline builtins fixture on both targets, then the invariant-9
# todod pairs (after unit 1). Every step prints STEP <name> exit=<code>; the log ends with EXIT=0.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
T=$S/target-s12-p2
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
step() { echo "STEP $1 exit=$2"; }
cd /home/timo/projects/telos || exit 9
df -h / | tail -1

echo "== inline fixture, rust"
$T/debug/ignite test $S/s12-p2-builtins.ignite --out $T/ignite-test --target-dir $T/ignite-test/cargo 2>&1 | tail -20
step fixture-rust ${pipestatus[1]}
echo "== inline fixture, llvm"
$T/debug/ignite test $S/s12-p2-builtins.ignite --target llvm --out $T/ignite-test --runtime $T/ignite-test/rt/debug/libignite_rt.a 2>&1 | tail -20
step fixture-llvm ${pipestatus[1]}

echo "== todod pairs after unit 1"
zsh $S/s12-p2-todod.sh unit1 3
step todod $?
echo "EXIT=0"
