#!/usr/bin/env bash
# Disk request of 2026-11-05 09:2x (at most one tree's target built, s17u6/ under 8G): stop the
# cold mode once the candidate's first cold gate is recorded, before the harness's retry
# rebuilds the baseline beside the candidate's target; then clean the candidate's target.
set -u
P=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/s17u6/perf17
T=/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-s17-u6
until [ -e "$P/DONE" ] || grep '"label": "candidate"' "$P/evidence/records.jsonl" | grep -- '-cold-1"' | grep -q '"unit": "gate-cold"'; do
  timeout 5 tail -f /dev/null
done
if [ ! -e "$P/DONE" ]; then
  echo "STOP_RETRY $(date +%T)"
  kill -TERM -- -"$(tr -d ' ' < "$P/mode.pgid")"
fi
until [ -e "$P/DONE" ]; do timeout 5 tail -f /dev/null; done
for d in "$P"/builds/baseline-*/ "$P"/builds/candidate-*/; do
  [ -d "$d" ] && (cd "$T" && cargo clean --target-dir "$d" 2>&1 | tail -1)
done
du -sh "$P/.."
echo "STOP_RETRY_END $(date +%T)"
