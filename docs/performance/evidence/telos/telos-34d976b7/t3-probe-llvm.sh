#!/bin/sh
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
T=$S/target-i/debug/telos
SRC=${1:-/home/timo/projects/telos/crates/telos/tests/fixtures/std_probe.telos}
NAME=${2:-std_probe}
cd /home/timo/projects/telos
$T build "$SRC" --target llvm --out $S/t3-out-llvm --runtime /home/timo/projects/telos/target/debug/libtelos_rt.a || exit 1
python3 -I $S/t3-run.py $S/t3-out-llvm/bin/$NAME > $S/t3-llvm.out 2>&1
echo "llvm rc=$?"
cat $S/t3-llvm.out
