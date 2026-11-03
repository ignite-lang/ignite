#!/bin/zsh
# usage: slice9check.sh <run-id> : time the shrunk boxed test alone on both targets, then run
# the slice9:: file of crates/ignite/tests in the P4 target dir.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
export CARGO_TARGET_DIR=$S/target-p4
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
ulimit -c 0
cd /home/timo/projects/telos || exit 97
run=$1
L=$S/p4-logs/boxed-after-$run.log
: >"$L"
F=crates/ignite/tests/fixtures/slice9/regions.ignite
for t in rust llvm; do
  for i in 1 2; do
    s=$(date +%s.%N)
    $S/target-p4/debug/ignite test $F "boxed values" --target $t --out $S/target-p4/ignite-slice9/regions --runtime $S/target-p4/ignite-test/rt/debug/libignite_rt.a --target-dir $S/target-p4/ignite-slice9/regions/cargo >>"$L" 2>&1
    e=$(date +%s.%N)
    echo "RUN target=$t rep=$i wall=$(echo "$e - $s" | bc) load=$(cut -d' ' -f1 /proc/loadavg)" >>"$L"
  done
done
for t in rust llvm; do
  s=$(date +%s.%N)
  $S/target-p4/debug/ignite test $F --target $t --out $S/target-p4/ignite-slice9/regions --runtime $S/target-p4/ignite-test/rt/debug/libignite_rt.a --target-dir $S/target-p4/ignite-slice9/regions/cargo >>"$L" 2>&1
  e=$(date +%s.%N)
  echo "RUN-ALL target=$t wall=$(echo "$e - $s" | bc) load=$(cut -d' ' -f1 /proc/loadavg)" >>"$L"
done
echo "EXIT=0" >>"$L"
L2=$S/p4-logs/slice9-file-$run.log
: >"$L2"
echo "START load=$(cut -d' ' -f1 /proc/loadavg)" >>"$L2"
s=$(date +%s.%N)
cargo test -p ignite --test main slice9:: >>"$L2" 2>&1
rc=$?
e=$(date +%s.%N)
echo "FILE slice9 wall=$(echo "$e - $s" | bc) exit=$rc load=$(cut -d' ' -f1 /proc/loadavg)" >>"$L2"
echo "EXIT=$rc" >>"$L2"
