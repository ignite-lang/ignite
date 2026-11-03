#!/bin/zsh
# P4: gate-time measurement for the AGENTS.md slice-10 row. Usage: p4_gate.sh LOGNAME
export CARGO_TARGET_DIR=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/target-k
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_INCREMENTAL=0
cd /home/timo/projects/telos || exit 1
LOG=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/$1.log
RT=$CARGO_TARGET_DIR/telos-test/rt/debug/libtelos_rt.a
PROG=/home/timo/.cache/scratch/34d976b7-453b-445c-bc5b-857b9e81d67a/gate-programs

now() { date +%s.%N }
elapsed() { printf '%.1f' $(( $2 - $1 )) }

gate() {
  local label=$1
  local t0=$(now)
  local t=$(now); cargo fmt --all -- --check > /dev/null 2>&1; echo "$label fmt EXIT=$? $(elapsed $t $(now)) s"
  t=$(now); cargo clippy --workspace --all-targets -- -D warnings > /dev/null 2>&1; echo "$label clippy EXIT=$? $(elapsed $t $(now)) s"
  t=$(now); cargo test --workspace --no-fail-fast > $LOG.$label.test 2>&1; echo "$label test EXIT=$? $(elapsed $t $(now)) s"
  t=$(now); task test-telos > $LOG.$label.telos 2>&1; echo "$label test-telos EXIT=$? $(elapsed $t $(now)) s"
  echo "$label gate total $(elapsed $t0 $(now)) s"
}

{
  echo "START $(date +%T)"
  ulimit -c 0
  uptime
  df -h / | tail -1
  cargo clean > /dev/null 2>&1
  export SCCACHE_RECACHE=1
  gate cold
  unset SCCACHE_RECACHE
  touch crates/telos-check/src/lib.rs
  gate warm
  t=$(now); cargo test --workspace --no-run > $LOG.norun 2>&1; echo "no-run warm EXIT=$? $(elapsed $t $(now)) s"
  touch crates/telos-check/src/lib.rs
  t=$(now); cargo test --workspace --no-run > $LOG.norun2 2>&1; echo "no-run after touch EXIT=$? $(elapsed $t $(now)) s"
  grep -o 'Executable .*' $LOG.norun2 | sed 's/Executable .* (\(.*\))/\1/' > $LOG.exes
  echo "test executables: $(wc -l < $LOG.exes), $(du -ch $(cat $LOG.exes) | tail -1 | cut -f1)"
  rm -rf $PROG
  mkdir -p $PROG
  TELOS=$CARGO_TARGET_DIR/debug/telos
  for profile in dev release; do
    flag=""
    [[ $profile == release ]] && flag="--release"
    t=$(now); $TELOS build examples/todo/todo.telos --out $PROG/rust-$profile --target-dir $PROG/rust-$profile-target $flag > /dev/null 2>&1; echo "todod rust $profile EXIT=$? $(elapsed $t $(now)) s"
    t=$(now); $TELOS build examples/todo/todo.telos --target llvm --runtime $RT --out $PROG/llvm-$profile $flag > /dev/null 2>&1; echo "todod llvm $profile EXIT=$? $(elapsed $t $(now)) s"
    t=$(now); $TELOS build examples/todo/todo.telos --target llvm --runtime $RT --out $PROG/llvm-$profile-2 $flag --verbose > $LOG.llvm-$profile.txt 2>&1; echo "todod llvm $profile (verbose, second out) EXIT=$? $(elapsed $t $(now)) s"
  done
  uptime
  echo "END $(date +%T)"
  echo "EXIT=0"
} > "$LOG" 2>&1
