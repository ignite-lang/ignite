#!/bin/zsh
# usage: fullgate.sh
# One full `task gate` of the before tree (apply suites included) in its target dir after
# cold.sh; slice 13's record: wall time, exit status, counts, end-to-end time. The after tree's
# full gate is the merge gate (logs/merge-gate-after.log), run in the worktree's own target/.
source /home/timo/.cache/scratch/c7fc37f1-9791-4466-8333-da801f43672c/s14/perf/scripts/env.sh
use_tree before
log=$LOGS/fullgate-before.log
: >"$log"; : >"$log.summary"
echo "START $(date -Is) tree=before load=$(cat /proc/loadavg) free=$(free_gb)G" | tee -a "$log.summary"
timed "$log" before full-task-gate 0 task gate
finish "$log"
