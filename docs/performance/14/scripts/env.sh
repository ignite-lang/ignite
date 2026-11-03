# Shared setup of the slice-14 gate-time scripts (docs/slice.md 18.18); sourced by zsh.
# Every script runs inside slot.sh, which exports CARGO_INCREMENTAL=0,
# CARGO_PROFILE_DEV_DEBUG=line-tables-only, RUST_TEST_THREADS=4 and CARGO_BUILD_JOBS=8.
zmodload zsh/datetime
S=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c
P=$S/s14/perf
LOGS=$P/logs
BUILDS=$P/builds
AFTER_TREE=/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-s14-u5b
mkdir -p $LOGS $BUILDS
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
ulimit -c 0
FAILED=0
FAILED_STEPS=""
DESIGN=examples/terminal/terminal.design.ignite
TODO=examples/todo/todo.ignite
TERMINAL=examples/terminal/terminal.ignite

# use_tree before|after|after-cold : cd into the tree and choose its target dir.
# before: $P/target-before. after: the worktree's own target/ (CARGO_TARGET_DIR unset), warm
# from the merge gate. after-cold: $P/target-after, used only by the after tree's cold gate and
# emptied with cargo clean after it (session disk budget 15G).
use_tree() {
  case $1 in
    before) TREE=$P/before; export CARGO_TARGET_DIR=$P/target-before ;;
    after) TREE=$AFTER_TREE; unset CARGO_TARGET_DIR ;;
    after-cold) TREE=$AFTER_TREE; export CARGO_TARGET_DIR=$P/target-after ;;
    *) echo "use_tree: unknown tree $1"; exit 98 ;;
  esac
  cd "$TREE" || exit 97
  local t=${CARGO_TARGET_DIR:-$TREE/target}
  IGNITE=$t/debug/ignite
  RT_DEV=$t/ignite-test/rt/debug/libignite_rt.a
  RT_REL=$t/ignite-test/rt/release/libignite_rt.a
}

load1() { cut -d' ' -f1 /proc/loadavg; }

free_gb() { df -BG --output=avail / | tail -1 | tr -dc 0-9; }

# guard : no build starts under 40G free; wait until there is room
guard() {
  while [[ $(free_gb) -lt 40 ]]; do
    echo "WAITDISK $(date +%T) free=$(free_gb)G"
    timeout 30 tail -f /dev/null
  done
}

# timed LOG TREE LABEL PAIR cmd... : command output to LOG; one STEP line to stdout and LOG.summary
timed() {
  local log=$1 tree=$2 label=$3 pair=$4; shift 4
  local lb la t0 t1 rc
  guard
  echo "=== $tree $label $pair: $*" >>"$log"
  lb=$(load1)
  t0=$EPOCHREALTIME
  "$@" >>"$log" 2>&1
  rc=$?
  t1=$EPOCHREALTIME
  la=$(load1)
  printf 'STEP tree=%s label=%s pair=%s secs=%.3f load_before=%s load_after=%s exit=%s\n' \
    "$tree" "$label" "$pair" $(( t1 - t0 )) "$lb" "$la" "$rc" | tee -a "$log.summary"
  if [[ $rc -ne 0 ]]; then
    FAILED=$((FAILED + 1))
    FAILED_STEPS="$FAILED_STEPS $tree/$label/$pair=$rc"
  fi
  return $rc
}

# gate LOG TREE PAIR KIND : the gate of 18.18 with the apply suites skipped, each step timed,
# then a STEP line for the whole gate whose load is the load at its start
gate() {
  local log=$1 tree=$2 pair=$3 kind=$4
  local lb t0 t1 rc=0
  guard
  lb=$(load1)
  t0=$EPOCHREALTIME
  timed "$log" $tree gate-$kind-fmt $pair cargo fmt --all -- --check || rc=1
  timed "$log" $tree gate-$kind-clippy $pair cargo clippy --workspace --all-targets -- -D warnings || rc=1
  timed "$log" $tree gate-$kind-test $pair cargo test --workspace -- --skip apply || rc=1
  timed "$log" $tree gate-$kind-test-ignite $pair task test-ignite || rc=1
  t1=$EPOCHREALTIME
  printf 'STEP tree=%s label=%s pair=%s secs=%.3f load_before=%s load_after=%s exit=%s\n' \
    "$tree" "gate-$kind" "$pair" $(( t1 - t0 )) "$lb" "$(load1)" "$rc" | tee -a "$log.summary"
}

# clean DIR : remove a scratch build directory the way the brief asks, with cargo clean
clean() {
  [[ -e $1 ]] && cargo clean -q --target-dir "$1" >/dev/null 2>&1
  [[ -e $1 ]] && { echo "clean: $1 still exists"; exit 96; }
  return 0
}

# clean_build DIR : remove one throwaway program or test build dir of pass.sh (ignite's --out and
# its --target-dir, created by this script under $BUILDS). Cargo refuses them (no CACHEDIR.TAG),
# so they go with rm, guarded to $BUILDS; the trees' target dirs only ever go through clean.
clean_build() {
  [[ $1 == $BUILDS/?* ]] || { echo "clean_build: $1 is not under $BUILDS"; exit 96; }
  rm -rf -- "$1"
}

# finish LOG : the final line of a script, EXIT= the number of failed steps and their names
finish() {
  echo "EXIT=$FAILED failed_steps=[${FAILED_STEPS# }] free=$(free_gb)G" | tee -a "$1.summary"
  return $FAILED
}
