#!/bin/zsh
# usage: time-ignite-test.sh : per-invocation cost of ignite test on both targets (warm cargo dir)
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/b11-scripts/env.sh
F=$REPO/crates/ignite/tests/fixtures/tests
OUT=$BUILDS/time-it
mkdir -p $OUT
t() { local label=$1; shift; local t0=$(date +%s.%N); "$@" >/dev/null 2>&1; local rc=$?; local t1=$(date +%s.%N); printf '%-28s %6.2f s exit=%s\n' $label $(echo "$t1 - $t0" | bc) $rc; }
t rust-basics-1 $IGNITE test $F/basics.ignite --out $OUT/rust --target-dir $OUT/cargo
t rust-basics-2 $IGNITE test $F/basics.ignite --out $OUT/rust --target-dir $OUT/cargo
t rust-basics-3-filter $IGNITE test $F/basics.ignite passes --out $OUT/rust --target-dir $OUT/cargo
t rust-slow $IGNITE test $F/slow.ignite --timeout 1 --out $OUT/rust --target-dir $OUT/cargo
t rust-basics-4 $IGNITE test $F/basics.ignite --out $OUT/rust --target-dir $OUT/cargo
t rust-unnamed $IGNITE test $F/unnamed_a.ignite $F/unnamed_b.ignite --out $OUT/rust --target-dir $OUT/cargo
t rust-overflow-dev $IGNITE test $F/overflow.ignite --out $OUT/rust --target-dir $OUT/cargo
t rust-overflow-rel $IGNITE test $F/overflow.ignite --release --out $OUT/rust --target-dir $OUT/cargo
t rust-overflow-rel-2 $IGNITE test $F/overflow.ignite --release --out $OUT/rust --target-dir $OUT/cargo
t rust-broken $IGNITE test $F/broken.ignite --out $OUT/rust --target-dir $OUT/cargo
t rust-targets $IGNITE test $F/targets.ignite --out $OUT/rust --target-dir $OUT/cargo
t llvm-basics-1 $IGNITE test $F/basics.ignite --target llvm --out $OUT/llvm --runtime $RT
t llvm-basics-2 $IGNITE test $F/basics.ignite --target llvm --out $OUT/llvm --runtime $RT
t llvm-slow $IGNITE test $F/slow.ignite --timeout 1 --target llvm --out $OUT/llvm --runtime $RT
t llvm-overflow-dev $IGNITE test $F/overflow.ignite --target llvm --out $OUT/llvm --runtime $RT
t llvm-overflow-rel $IGNITE test $F/overflow.ignite --release --target llvm --out $OUT/llvm --runtime $RT
t llvm-targets $IGNITE test $F/targets.ignite --target llvm --out $OUT/llvm --runtime $RT
echo "EXIT=0"
