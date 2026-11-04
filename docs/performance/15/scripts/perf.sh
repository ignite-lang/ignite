#!/usr/bin/env bash
set -u
cd /home/timo/.local/state/worktree/trees/codewandler/ignite-lang/ignite-s15-claims
P=/home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/perf15
mkdir -p "$P/sources" "$P/builds" "$P/evidence"
cargo build -q -p ignite-perf || exit 1
H=target/debug/ignite-perf
ARGS=(--baseline 296f849 --candidate c20f04b --comparison "" --sources "$P/sources" --builds "$P/builds" --evidence "$P/evidence")
for mode in cold warm programs cli; do
  echo "MODE $mode $(date +%T) load $(cut -d' ' -f1 /proc/loadavg)"
  "$H" "$mode" "${ARGS[@]}"
  echo "MODE_EXIT $mode=$? $(date +%T)"
done
"$H" summarize "${ARGS[@]}"
echo "SUMMARIZE_EXIT=$?"
bash /home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/s15/u6c-extra.sh
echo "EXTRA_EXIT=$?"
