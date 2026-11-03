#!/bin/zsh
# P2 slice 12 chain 8: rebuild ignite with the rustgen `Exposure` import fix, the rustgen tests
# (goldens must stay byte-identical), `ignite build` of the design file on both targets, then
# the split of the todod rise: Rust (ignite-io, ignite-std, program) and LLVM (clang on std.ll,
# `ignite check`) on the base and the new tree. Every step prints STEP <name> exit=<code>.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
T=$S/target-s12-p2
B=$S/target-s12-before-p2
export CARGO_TARGET_DIR=$T
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
quiet() { grep -v '^\s*Compiling\|^\s*Blocking\|^\s*Locking\|^\s*Finished\|^\s*Checking\|^test .* ok$\|^\s*$\|^running \|Doc-tests\|^\s*Running'; true }
step() { echo "STEP $1 exit=$2"; }
load1() { cut -d' ' -f1 /proc/loadavg }
secs() { local t0=$(date +%s.%N); "$@" > /dev/null 2>&1; local rc=$?; local t1=$(date +%s.%N); printf '%.3f' $(echo "$t1 - $t0" | bc); return $rc }
cd /home/timo/projects/telos || exit 9
df -h / | tail -1

echo "== build ignite"
cargo build -p ignite 2>&1 | quiet
step build ${pipestatus[1]}
echo "== rustgen tests"
cargo test -p ignite-rustgen 2>&1 | quiet
step rustgen-tests ${pipestatus[1]}
git status --short crates/ignite-rustgen/tests/golden/ | grep -v 'targets\.' ; echo "(golden status above: only targets.* are expected, P1's new files)"

echo "== design file, rust"
rm -rf $S/s12-p2-design-rust
$T/debug/ignite build examples/terminal/terminal.design.ignite --out $S/s12-p2-design-rust/project --target-dir $S/s12-p2-design-rust/target 2>&1 | quiet | tail -12
step design-rust ${pipestatus[1]}
echo "== design file, llvm"
rm -rf $S/s12-p2-design-llvm
$T/debug/ignite build examples/terminal/terminal.design.ignite --target llvm --out $S/s12-p2-design-llvm --runtime $T/debug/libignite_rt.a 2>&1 | tail -12
step design-llvm ${pipestatus[1]}

echo "== split, rust: cold builds of ignite-io, ignite-std, the program (SCCACHE_RECACHE=1)"
export SCCACHE_RECACHE=1
for i in 1 2; do
  for which in base new; do
    for profile in dev release; do
      p=$S/s12-p2-bench-unit1cold/$which-rust-$profile-1/project
      t=$S/s12-p2-split-$which-$profile-$i
      rm -rf $t
      flag=(); [ $profile = release ] && flag=(--release)
      io=$(secs cargo build --manifest-path $p/Cargo.toml --target-dir $t $flag -p ignite-io)
      std=$(secs cargo build --manifest-path $p/Cargo.toml --target-dir $t $flag -p ignite-std)
      prog=$(secs cargo build --manifest-path $p/Cargo.toml --target-dir $t $flag)
      echo "SPLIT rust $which $profile run=$i io=$io std=$std program=$prog load=$(load1)"
      rm -rf $t
    done
  done
done
unset SCCACHE_RECACHE

echo "== split, llvm: clang on std.ll and ignite check (3 runs)"
for i in 1 2 3; do
  for which in base new; do
    l=$S/s12-p2-bench-unit1cold/$which-llvm-dev-1/project/llvm
    o0=$(secs clang -c -O0 $l/std.ll -o $S/s12-p2-split-std.o)
    o2=$(secs clang -c -O2 $l/std.ll -o $S/s12-p2-split-std.o)
    p2=$(secs clang -c -O2 $l/program.ll -o $S/s12-p2-split-prog.o)
    if [ $which = base ]; then tree=$S/s12-before; bin=$B/debug/ignite; else tree=/home/timo/projects/telos; bin=$T/debug/ignite; fi
    chk=$(cd $tree && secs $bin check examples/todo/todo.ignite)
    emit=$(cd $tree && secs $bin emit examples/todo/todo.ignite)
    echo "SPLIT llvm $which run=$i clang_std_O0=$o0 clang_std_O2=$o2 clang_program_O2=$p2 check=$chk emit_rust=$emit load=$(load1)"
  done
done
rm -f $S/s12-p2-split-std.o $S/s12-p2-split-prog.o
echo "EXIT=0"
