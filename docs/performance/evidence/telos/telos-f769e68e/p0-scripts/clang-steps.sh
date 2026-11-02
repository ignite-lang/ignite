#!/bin/zsh
# usage: clang-steps.sh <run-id> <reps>
# Reruns by hand, timed, the clang -c and link commands ignite build uses
# (crates/ignite/src/llvm_build.rs: COMPILE_FLAGS, LINK_FLAGS, RELEASE_LINK_FLAGS, RUNTIME_LIBS)
# on the .ll files of p0-build/run-<run-id>/llvm-{dev,release}/llvm.
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/p0-scripts/env.sh
run=$1; reps=${2:-3}
B=$S/p0-build/run-$run
log=$LOGS/clang-steps-$run.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) run=$run load=$(cat /proc/loadavg)" | tee -a "$log.summary"
triple=$(clang -print-target-triple)
echo "triple=$triple" | tee -a "$log.summary"
RT_DEV=$CARGO_TARGET_DIR/ignite-test/rt/debug/libignite_rt.a
RT_REL=$CARGO_TARGET_DIR/ignite-test/rt/release/libignite_rt.a
for profile in dev release; do
  opt=-O0; [[ $profile == release ]] && opt=-O2
  rt=$RT_DEV; [[ $profile == release ]] && rt=$RT_REL
  ir=$B/llvm-$profile/llvm
  work=$B/clang-steps-$profile
  mkdir -p $work
  for ll in $ir/*.ll; do
    name=${ll:t:r}
    echo "LINES $profile $name $(wc -l <$ll)" | tee -a "$log.summary"
  done
  for rep in $(seq 1 $reps); do
    for ll in $ir/*.ll; do
      name=${ll:t:r}
      timed "$log" "$profile:clang-c:$name:$rep" clang -c $opt -g -ffunction-sections -fdata-sections -Werror=override-module --target=$triple $ll -o $work/$name.o
    done
    strip=(); [[ $profile == release ]] && strip=(-Wl,--strip-debug)
    timed "$log" "$profile:link:todod:$rep" clang -o $work/todod --target=$triple $work/main_todod.o $work/program.o $work/std.o $rt -Wl,--gc-sections $strip -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
  done
  ls -l $work/*.o $work/todod >>"$log"
done
echo "EXIT=0" | tee -a "$log.summary"
