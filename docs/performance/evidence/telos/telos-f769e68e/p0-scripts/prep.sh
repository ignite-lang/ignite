#!/bin/zsh
# Untimed preparation: release runtime archive, a probe LLVM build for the IR analysis.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p0-scripts/env.sh
log=$LOGS/prep.log
: >"$log"
echo "START $(date -Is) load=$(cat /proc/loadavg)" >>"$log"
timed "$log" rt-release cargo build -q --release -p ignite-rt --features runner --target-dir $CARGO_TARGET_DIR/ignite-test/rt
ls -l $CARGO_TARGET_DIR/ignite-test/rt/release/libignite_rt.a $CARGO_TARGET_DIR/ignite-test/rt/debug/libignite_rt.a >>"$log" 2>&1
mkdir -p $S/p0-build/probe
timed "$log" probe-llvm-dev $CARGO_TARGET_DIR/debug/ignite build examples/todo/todo.ignite --target llvm --runtime $CARGO_TARGET_DIR/ignite-test/rt/debug/libignite_rt.a --out $S/p0-build/probe/llvm-dev
timed "$log" probe-rust-dev $CARGO_TARGET_DIR/debug/ignite build examples/todo/todo.ignite --out $S/p0-build/probe/rust-dev --target-dir $S/p0-build/probe/rust-dev-target
ls -R $S/p0-build/probe/llvm-dev >>"$log"
find $S/p0-build/probe/rust-dev -maxdepth 3 -not -path '*/target/*' >>"$log"
wc -l $S/p0-build/probe/llvm-dev/llvm/*.ll >>"$log"
python3 -I $S/p0-scripts/ir_reach.py $S/p0-build/probe/llvm-dev/llvm >$LOGS/ir-reach-probe.log 2>&1
echo "EXIT=0" >>"$log"
