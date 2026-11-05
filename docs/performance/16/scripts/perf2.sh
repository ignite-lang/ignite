#!/usr/bin/env bash
# Second half of the 20.12 run: perf.sh's cold mode completed, its later modes found no harness
# binary (the worktree target was cleaned for the disk budget while cold ran). Same arguments.
set -u
T=/home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-s16-u6
P=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/u6/perf16
cd "$T"
cargo build -q -p ignite-perf || exit 1
H=$T/target/debug/ignite-perf
ARGS=(--baseline 31b738b96b67 --candidate 426449e9c67f --comparison "" --sources "$P/sources" --builds "$P/builds" --evidence "$P/evidence")
for mode in warm programs cli; do
  echo "MODE $mode $(date +%T) load $(cut -d' ' -f1 /proc/loadavg) free $(df -BG --output=avail / | tail -1)"
  "$H" "$mode" "${ARGS[@]}"
  echo "MODE_EXIT $mode=$? $(date +%T) builds $(du -sh "$P/builds" | cut -f1)"
done
"$H" summarize "${ARGS[@]}"
echo "SUMMARIZE_EXIT=$?"
rm -f "$P/SKIP_EXTRA"
bash "$P/extra.sh"
echo "EXTRA_EXIT=$?"
