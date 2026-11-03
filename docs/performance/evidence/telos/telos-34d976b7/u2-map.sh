#!/bin/sh
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-j CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
TAG=$1
cd /home/timo/projects/telos || exit 1
cargo build -q -p telos-rt || exit 1
O=$S/u2-aft12/llvm/llvm
mkdir -p $S/u2-map-$TAG
cp $CARGO_TARGET_DIR/debug/libtelos_rt.a $S/u2-map-$TAG/
clang -o $S/u2-map-$TAG/terminald $O/main_terminald.o $O/program.o $O/std.o $S/u2-map-$TAG/libtelos_rt.a -Wl,--gc-sections -Wl,-Map,$S/u2-map-$TAG/map.txt -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
stat -c '%s %n' $S/u2-map-$TAG/terminald
grep -o 'libtelos_rt.a([^)]*)' $S/u2-map-$TAG/map.txt | sort -u > $S/u2-map-$TAG/members.txt
wc -l < $S/u2-map-$TAG/members.txt
echo EXIT=0
