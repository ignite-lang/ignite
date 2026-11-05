#!/usr/bin/env bash
# Disk watch beside a harness mode: every 20 s the size of each side's target and of the builds
# root. A side target over 12G stops the mode (its process group, from mode.pgid). With
# CLEAN_BASELINE=1 the baseline's target is
# cleaned with cargo clean once the records show a cold row of the candidate (the baseline's cold
# gate is done by then). Ends when DONE exists.
set -u
P=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/s17u6/perf17
T=/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-s17-u6
cleaned=0
while [ ! -e "$P/DONE" ]; do
  line="SIZE $(date +%T) load $(cut -d' ' -f1 /proc/loadavg)"
  over=0
  for d in "$P"/builds/baseline-* "$P"/builds/candidate-*; do
    [ -d "$d" ] || continue
    k=$(du -sk "$d" 2>/dev/null | cut -f1)
    line="$line $(basename "$d")=$((k / 1048576))G"
    [ "$k" -gt 12582912 ] && over=1
  done
  total=$(du -sk "$P/builds" "$T/target" 2>/dev/null | awk '{s+=$1} END {print int(s/1048576)}')
  echo "$line total=${total}G"
  if [ "$over" = 1 ] && [ -e "$P/mode.pgid" ]; then
    echo "STOP $(date +%T) a side target passed 12G"
    kill -TERM -- -"$(tr -d ' ' < "$P/mode.pgid")"
    touch "$P/STOPPED"
  fi
  if [ "${CLEAN_BASELINE:-0}" = 1 ] && [ "$cleaned" = 0 ] && grep '"label": "candidate"' "$P/evidence/records.jsonl" | grep -q -- '-cold-'; then
    b=$(ls -d "$P"/builds/baseline-*/ 2>/dev/null | head -1)
    if [ -n "$b" ]; then
      echo "CLEAN_BASELINE $(date +%T) $b"
      (cd "$T" && cargo clean --target-dir "$b" 2>&1 | tail -1)
      cleaned=1
    fi
  fi
  timeout 20 tail -f /dev/null
done
echo "WATCH_END $(date +%T)"
