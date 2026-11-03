#!/usr/bin/env bash
# usage: rtvar.sh <label> [env assignments...]; builds telos-rt --release with the env, links, prints sizes and build time
set -u
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
L=$1; shift
cd /home/timo/projects/telos
TD=$S/target-rtvar-$L
s=$(date +%s.%N)
env CARGO_TARGET_DIR=$TD CARGO_INCREMENTAL=0 RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= "$@" cargo build -q -p telos-rt --release || exit 1
e=$(date +%s.%N)
printf '%s rt-build=%.1fs archive=%s\n' "$L" $(echo "$e - $s" | bc) $(stat -c %s $TD/release/libtelos_rt.a)
bash $S/link.sh $S/fs/release $TD/release/libtelos_rt.a rel-$L -Wl,--gc-sections
bash $S/link.sh $S/fs/release $TD/release/libtelos_rt.a rel-$L-sd -Wl,--gc-sections -Wl,--strip-debug
