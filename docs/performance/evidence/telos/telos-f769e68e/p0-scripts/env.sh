S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
TREE=$S/s11-p0-tree
LOGS=$S/p0-logs
export CARGO_TARGET_DIR=$S/target-p0
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
cd "$TREE" || exit 97

load1() { cut -d' ' -f1 /proc/loadavg; }

# timed LOG LABEL cmd... : appends command output to LOG, prints one STEP line to stdout and to LOG.summary
timed() {
  local log=$1 label=$2; shift 2
  local lb la t0 t1 rc dt
  lb=$(load1)
  t0=$(date +%s.%N)
  "$@" >>"$log" 2>&1
  rc=$?
  t1=$(date +%s.%N)
  la=$(load1)
  dt=$(echo "$t1 - $t0" | bc)
  printf 'STEP %s secs=%.2f load_before=%s load_after=%s exit=%s\n' "$label" "$dt" "$lb" "$la" "$rc" | tee -a "$log.summary"
  return $rc
}

# waitload MAX TIMEOUT : wait until the 1-minute load average is below MAX (10 s polls), at most TIMEOUT s
waitload() {
  local max=$1 limit=${2:-7200} t0=$SECONDS
  while (( $(load1) >= max )); do
    (( SECONDS - t0 > limit )) && { echo "waitload timeout load=$(load1)"; return 1; }
    sleep 10
  done
  return 0
}
