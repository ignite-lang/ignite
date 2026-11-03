#!/bin/zsh
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
export CARGO_TARGET_DIR=$S/target-12b-r2
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
ulimit -c 0
cd /home/timo/projects/telos || exit 97
LOG=$S/r2-build3.log
: >$LOG
echo "START $(date -Is) load=$(cat /proc/loadavg)" >>$LOG
IGNITE_BLESS=1 cargo test -p ignite-rustgen --test main -- golden:: >>$LOG 2>&1
echo "BLESS rc=$? $(date -Is)" >>$LOG
cargo test -p ignite-rustgen --test main -- reachable:: units:: golden:: >>$LOG 2>&1
rc=$?
echo "EXIT=$rc $(date -Is)" >>$LOG
exit $rc
