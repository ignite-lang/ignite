#!/bin/zsh
# usage: todod.sh <run-id>
# Cold builds of examples/todo/todo.ignite: fresh --out and --target-dir, SCCACHE_RECACHE=1,
# dev and release, both targets. Each build once plain (reported time) and once under
# strace (execve/exit_group timeline for the per-step split).
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p0-scripts/env.sh
run=$1
B=$S/p0-build/run-$run
mkdir -p $B
log=$LOGS/todod-$run.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) run=$run load=$(cat /proc/loadavg)" | tee -a "$log.summary"
IGNITE=$CARGO_TARGET_DIR/debug/ignite
RT_DEV=$CARGO_TARGET_DIR/ignite-test/rt/debug/libignite_rt.a
RT_REL=$CARGO_TARGET_DIR/ignite-test/rt/release/libignite_rt.a
export SCCACHE_RECACHE=1
STRACE=(strace --seccomp-bpf -f -ttt -e trace=execve,exit_group -s 4000)

for profile in dev release; do
  rel=(); [[ $profile == release ]] && rel=(--release)
  rt=$RT_DEV; [[ $profile == release ]] && rt=$RT_REL

  out=$B/rust-$profile
  timed "$log" "rust-$profile" $IGNITE build examples/todo/todo.ignite $rel --out $out --target-dir $out-target
  out=$B/rust-$profile-strace
  timed "$log" "rust-$profile-strace" $STRACE -o $LOGS/todod-$run-rust-$profile.strace $IGNITE build examples/todo/todo.ignite $rel --out $out --target-dir $out-target

  out=$B/llvm-$profile
  timed "$log" "llvm-$profile" $IGNITE build examples/todo/todo.ignite $rel --target llvm --runtime $rt --out $out
  out=$B/llvm-$profile-strace
  timed "$log" "llvm-$profile-strace" $STRACE -o $LOGS/todod-$run-llvm-$profile.strace $IGNITE build examples/todo/todo.ignite $rel --target llvm --runtime $rt --out $out
done
echo "EXIT=0" | tee -a "$log.summary"
