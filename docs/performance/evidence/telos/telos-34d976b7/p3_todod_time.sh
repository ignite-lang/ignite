#!/bin/zsh
# usage: p3_todod_time.sh <label>
# Builds telos and telos-rt into target-h, then times the LLVM todod build three times.
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
export CARGO_TARGET_DIR=$S/target-h
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
cd /home/timo/projects/telos
label=$1
s=$(date +%s.%N)
cargo build -q -p telos -p telos-rt 2>&1
st=$?
e=$(date +%s.%N)
printf "%s cargo build status=%d %.1f s\n" $label $st $(( e - s ))
if [ $st -ne 0 ]; then echo "EXIT=$st"; exit $st; fi
bin=$S/target-h/debug/telos
rt=$S/target-h/debug/libtelos_rt.a
for i in 1 2 3; do
  out=$S/p3-todod-$label-$i
  rm -rf $out
  s=$(date +%s.%N)
  $bin build examples/todo/todo.telos --target llvm --out $out --runtime $rt > $S/p3-todod-$label-$i.log 2>&1
  st=$?
  e=$(date +%s.%N)
  printf "%s todod llvm dev run%d status=%d %.2f s size=%s\n" $label $i $st $(( e - s )) "$(stat -c %s $out/bin/todod 2>/dev/null)"
done
for i in 1 2; do
  out=$S/p3-todod-$label-rel-$i
  rm -rf $out
  s=$(date +%s.%N)
  $bin build examples/todo/todo.telos --target llvm --release --out $out --runtime $rt > $S/p3-todod-$label-rel-$i.log 2>&1
  st=$?
  e=$(date +%s.%N)
  printf "%s todod llvm release run%d status=%d %.2f s size=%s\n" $label $i $st $(( e - s )) "$(stat -c %s $out/bin/todod 2>/dev/null)"
done
echo "EXIT=0"
