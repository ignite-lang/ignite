#!/bin/zsh
# usage: repro.sh <stem>... : run each repro fixture with ignite test on both targets
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/b11-scripts/env.sh
for stem in "$@"; do
  f=$BUILDS/repro/$stem.ignite
  echo "===== $stem rust"
  $IGNITE test $f --out $BUILDS/repro-out/$stem --target-dir $BUILDS/repro-out/cargo 2>&1 | grep -v '^\s*Compiling\|^\s*Finished\|^\s*Running' | head -60
  echo "exit=${pipestatus[1]}"
  echo "===== $stem llvm"
  $IGNITE test $f --target llvm --out $BUILDS/repro-out/$stem --runtime $RT 2>&1 | head -60
  echo "exit=${pipestatus[1]}"
done
echo "EXIT=0"
