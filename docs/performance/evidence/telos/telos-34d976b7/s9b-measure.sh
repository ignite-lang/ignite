#!/bin/zsh
# usage: s9b-measure.sh <label>   writes $S/s9b-<label>.* and a summary to $S/s9b-<label>.sum
ulimit -c 0
S=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a
L=$1
O=$S/s9b-$L
cd ${TREE:-/home/timo/projects/telos}
export CARGO_TARGET_DIR=${TD:-$S/target-k} CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=line-tables-only
SUM=$O.sum
: > $SUM

now() { date +%s.%N }
step() {
  local name=$1; shift
  local t0=$(now)
  "$@" > $O.$name.log 2>&1
  local rc=$?
  local t1=$(now)
  printf '%s\t%.1f\texit=%s\n' $name $(( t1 - t0 )) $rc >> $SUM
  return $rc
}
noexe() {
  grep -E '^ +Executable ' $O.$1.log | sed -E 's/.*\((.*)\)$/\1/' > $O.$1.exes
  printf '%s\texecutables=%s\tsize=%s\n' $1 $(wc -l < $O.$1.exes) "$(du -ch $(cat $O.$1.exes) | tail -1 | cut -f1)" >> $SUM
}

df -h / | tail -1 >> $SUM; uptime >> $SUM
cargo clean >> $O.clean.log 2>&1
step norun_cached cargo test --workspace --no-run
noexe norun_cached
step norun_warm cargo test --workspace --no-run
touch crates/telos-check/src/lib.rs
step norun_touch cargo test --workspace --no-run

cargo clean >> $O.clean.log 2>&1
export SCCACHE_RECACHE=1
t0=$(now)
step cold_fmt cargo fmt --all -- --check
step cold_clippy cargo clippy --workspace --all-targets -- -D warnings
step cold_test cargo test --workspace --no-fail-fast
step cold_telos task test-telos
t1=$(now)
printf 'gate_cold\t%.1f\n' $(( t1 - t0 )) >> $SUM
unset SCCACHE_RECACHE

touch crates/telos-check/src/lib.rs
t0=$(now)
step warm_fmt cargo fmt --all -- --check
step warm_clippy cargo clippy --workspace --all-targets -- -D warnings
step warm_test cargo test --workspace --no-fail-fast
step warm_telos task test-telos
t1=$(now)
printf 'gate_warm\t%.1f\n' $(( t1 - t0 )) >> $SUM

step telos_again task test-telos
grep -h '^test result' $O.cold_test.log | awk '{p+=$4; f+=$6; i+=$8} END {printf "cargo_tests\tpassed=%d failed=%d ignored=%d\n", p, f, i}' >> $SUM
grep -E '^ +Running ' $O.cold_test.log | grep -c 'tests/' | sed 's/^/integration_binaries\t/' >> $SUM
grep -E '^ +Running tests/' $O.cold_test.log | sed -E 's/.*\(.*deps\/([a-z_0-9]+)-.*/\1/' | sort | uniq -c | tr '\n' ' ' >> $SUM
echo >> $SUM
grep -h '^test result' $O.cold_telos.log >> $SUM
find $CARGO_TARGET_DIR/telos-test -type f \( -name telos-test \) >> $SUM
df -h / | tail -1 >> $SUM; uptime >> $SUM
echo EXIT=0 >> $SUM
