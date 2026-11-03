#!/bin/zsh
# usage: goldens.sh <log> [bless] : the rustgen and llvm golden tests, optionally re-blessed
source /home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6/b11-scripts/env.sh
log=$1
: >$log
cd $REPO || exit 97
if [[ $2 == bless ]]; then export IGNITE_BLESS=1; fi
cargo test -p ignite-rustgen --test main golden:: >>$log 2>&1; echo "RUSTGEN_EXIT=$?" >>$log
cargo test -p ignite-llvm --test main golden:: >>$log 2>&1; echo "LLVM_EXIT=$?" >>$log
echo "EXIT=0" >>$log
