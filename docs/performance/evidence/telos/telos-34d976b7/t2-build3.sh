#!/bin/sh
# Build terminal, todo and generics with the Rust target; one log each.
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
T=$S/target-j
R=/home/timo/projects/telos
G=$R/crates/telos-rustgen/tests/golden
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
unset CARGO_TARGET_DIR
rm -rf $S/t2-std-http
cp -r $R/std $S/t2-std-http
cp $R/crates/telos/tests/fixtures/http.telos $S/t2-std-http/protocol/http.telos
cd $R
timeout 900 $T/debug/telos build examples/terminal/terminal.telos --out $S/t2-out/terminal --target-dir $S/t2-out/tgt > $S/t2-b-terminal.log 2>&1
echo "terminal EXIT=$?"
timeout 900 $T/debug/telos build --std $S/t2-std-http examples/todo/todo.telos --out $S/t2-out/todo --target-dir $S/t2-out/tgt > $S/t2-b-todo.log 2>&1
echo "todo EXIT=$?"
timeout 900 $T/debug/telos build --std $G/std $G/generics.telos --out $S/t2-out/gen --target-dir $S/t2-out/tgt > $S/t2-b-gen.log 2>&1
echo "gen EXIT=$?"
