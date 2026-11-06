#!/usr/bin/env bash
# The generated files of the programs mode's first pass, per tree: line counts of the LLVM IR and
# the generated Rust std, and the executables each build wrote. Run after `programs`, before the
# builds root is cleaned.
# usage: sizes.sh EVIDENCE
set -u
P=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/s18w5u8/perf18
D=$(ls -d "$P"/builds/programs/*/ | head -1)
for side in baseline candidate; do
  for profile in dev release; do
    echo "== $side llvm $profile"
    wc -l "$D/1-$side-llvm-$profile"/llvm/*.ll | sed "s#$D##"
    ls -l "$D/1-$side-llvm-$profile"/bin | tail -n +2 | awk '{print "bin", $NF, $5}'
    echo "== $side rust $profile"
    wc -l "$D/1-$side-rust-$profile"/std/src/lib.rs "$D/1-$side-rust-$profile"/src/*.rs 2>/dev/null | sed "s#$D##"
    ls -l "$D/1-$side-rust-$profile"/bin 2>/dev/null | tail -n +2 | awk '{print "bin", $NF, $5}'
  done
done
