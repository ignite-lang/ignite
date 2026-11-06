#!/usr/bin/env bash
# docs/slice.md 22.15 through ignite-perf: one harness mode per call, same arguments every time.
# usage: [BASE=REF] [ROOT=DIR] mode.sh EVIDENCE MODE...   (EVIDENCE: the evidence directory's name under $P;
# ROOT holds sources/ and builds/, $P by default; run2 used ROOT=$P/run2-root, BASE=661e55b42)
set -u
P=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/s18w5u8/perf18
T=/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-s18-u8
H=$P/bin/ignite-perf
E=$P/$1
shift
BASE=${BASE:-421742b33}
ROOT=${ROOT:-$P}
ARGS=(--repo "$T" --baseline "$BASE" --candidate 661e55b42 --comparison "" --cold-retries 0 --sources "$ROOT/sources" --builds "$ROOT/builds" --evidence "$E")
cd "$T"
ps -o pgid= $$ > "$P/mode.pgid"
for mode in "$@"; do
  echo "MODE $mode $(date +%T) load $(cut -d' ' -f1 /proc/loadavg) free $(df -BG --output=avail / | tail -1 | tr -d ' ')"
  "$H" "$mode" "${ARGS[@]}"
  echo "MODE_EXIT $mode=$? $(date +%T) builds $(du -sh "$ROOT/builds" 2>/dev/null | cut -f1)"
done
