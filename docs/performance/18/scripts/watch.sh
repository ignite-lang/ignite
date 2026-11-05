#!/usr/bin/env bash
# Disk watch beside a harness mode: every 20 s the size of each side's target, of the builds
# root and of the unit's tree target. Scratch plus target over 14G stops the mode (its process
# group, from mode.pgid). With CLEAN_BASELINE=1 the baseline's target is cleaned with cargo clean
# once the records of EVIDENCE show a cold row of the candidate (the baseline's cold gate is done
# by then), so only one tree's gate target exists at a time. Ends when DONE exists.
# usage: watch.sh EVIDENCE
set -u
P=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/s18w5u8/perf18
T=/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-s18-u8
E=$P/$1
cleaned=0
while [ ! -e "$P/DONE" ]; do
  line="SIZE $(date +%T) load $(cut -d' ' -f1 /proc/loadavg) free $(df -BG --output=avail / | tail -1 | tr -d ' ')"
  for d in "$P"/builds/baseline-* "$P"/builds/candidate-*; do
    [ -d "$d" ] || continue
    k=$(du -sk "$d" 2>/dev/null | cut -f1)
    line="$line $(basename "$d")=$((k / 1048576))G"
  done
  total=$(du -sk "$P/.." "$T/target" 2>/dev/null | awk '{s+=$1} END {print int(s/1048576)}')
  echo "$line total=${total}G"
  if [ "$total" -ge 14 ] && [ -e "$P/mode.pgid" ]; then
    echo "STOP $(date +%T) scratch and target passed 14G"
    kill -TERM -- -"$(tr -d ' ' < "$P/mode.pgid")"
    touch "$P/STOPPED"
  fi
  if [ "${CLEAN_BASELINE:-0}" = 1 ] && [ "$cleaned" = 0 ] && [ -e "$E/records.jsonl" ] && grep '"label": "candidate"' "$E/records.jsonl" | grep -q -- '-cold-'; then
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
