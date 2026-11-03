#!/bin/zsh
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
export CARGO_TARGET_DIR=$S/target-12b-r2
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
ulimit -c 0
cd /home/timo/projects/telos || exit 97
LOG=$S/r2-build1.log
: >$LOG
echo "START $(date -Is) load=$(cat /proc/loadavg) free=$(df -k / | awk 'NR==2 {print int($4/1048576)}')G" >>$LOG
cargo build -p ignite-rustgen --tests >>$LOG 2>&1
rc=$?
echo "EXIT=$rc $(date -Is)" >>$LOG
exit $rc
