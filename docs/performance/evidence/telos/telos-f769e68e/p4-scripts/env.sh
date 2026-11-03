S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
LOGS=$S/p4-logs
BUILDS=$S/p4-build
mkdir -p $LOGS $BUILDS
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0

# use_tree before|after : cd into the tree and point CARGO_TARGET_DIR at its own target dir
use_tree() {
  case $1 in
    before) TREE=$S/s11-p4-before; export CARGO_TARGET_DIR=$S/target-p4-before ;;
    after)  TREE=$S/s11-p4-after;  export CARGO_TARGET_DIR=$S/target-p4-after ;;
    *) echo "use_tree: unknown tree $1"; exit 98 ;;
  esac
  cd "$TREE" || exit 97
  IGNITE=$CARGO_TARGET_DIR/debug/ignite
  RT_DEV=$CARGO_TARGET_DIR/ignite-test/rt/debug/libignite_rt.a
  RT_REL=$CARGO_TARGET_DIR/ignite-test/rt/release/libignite_rt.a
}

load1() { cut -d' ' -f1 /proc/loadavg; }

# timed LOG TREE LABEL PAIR cmd... : command output to LOG; one STEP line to stdout and LOG.summary
timed() {
  local log=$1 tree=$2 label=$3 pair=$4; shift 4
  local lb la t0 t1 rc dt
  lb=$(load1)
  t0=$(date +%s.%N)
  "$@" >>"$log" 2>&1
  rc=$?
  t1=$(date +%s.%N)
  la=$(load1)
  dt=$(echo "$t1 - $t0" | bc)
  printf 'STEP tree=%s label=%s pair=%s secs=%.2f load_before=%s load_after=%s exit=%s\n' "$tree" "$label" "$pair" "$dt" "$lb" "$la" "$rc" | tee -a "$log.summary"
  return $rc
}

# fresh DIR : refuse to reuse a directory that exists (never delete one)
fresh() {
  if [[ -e $1 ]]; then echo "fresh: $1 exists, refusing to reuse it"; exit 96; fi
  mkdir -p "$1"
}

# gate LOG TREE PAIR [cold] : the four gate steps, one after another, plus a TOTAL line
gate() {
  local log=$1 tree=$2 pair=$3 mode=${4:-warm}
  local T0 T1 lb
  lb=$(load1)
  T0=$(date +%s.%N)
  timed "$log" $tree gate-fmt $pair cargo fmt --all -- --check
  timed "$log" $tree gate-clippy $pair cargo clippy --workspace --all-targets -- -D warnings
  timed "$log" $tree gate-test $pair cargo test --workspace --no-fail-fast
  timed "$log" $tree gate-test-ignite $pair task test-ignite
  T1=$(date +%s.%N)
  printf 'STEP tree=%s label=gate-%s-total pair=%s secs=%.2f load_before=%s load_after=%s exit=0\n' "$tree" "$mode" "$pair" "$(echo "$T1 - $T0" | bc)" "$lb" "$(load1)" | tee -a "$log.summary"
}
