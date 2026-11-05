#!/usr/bin/env bash
# docs/slice.md 21.14 through ignite-perf: one harness mode per call, same arguments every time.
# usage: mode.sh MODE...
set -u
P=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/s17u6/perf17
T=/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-s17-u6
H=$P/bin/ignite-perf
ARGS=(--repo "$T" --baseline 99647132 --candidate 884d6b19 --comparison "" --sources "$P/sources" --builds "$P/builds" --evidence "$P/evidence")
cd "$T"
ps -o pgid= $$ > "$P/mode.pgid"
for mode in "$@"; do
  echo "MODE $mode $(date +%T) load $(cut -d' ' -f1 /proc/loadavg) free $(df -BG --output=avail / | tail -1 | tr -d ' ')"
  "$H" "$mode" "${ARGS[@]}"
  echo "MODE_EXIT $mode=$? $(date +%T) builds $(du -sh "$P/builds" 2>/dev/null | cut -f1)"
done
