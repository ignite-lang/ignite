#!/bin/zsh
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
export CARGO_TARGET_DIR=$S/target-12b-r1
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
cd /home/timo/projects/telos || exit 97
LOG=$S/s12b-r1-build1.log
: >$LOG
echo "START $(date -Is) load=$(cut -d' ' -f1 /proc/loadavg) free=$(df -k / | awk 'NR==2 {print int($4/1048576)}')G" >>$LOG
cargo build -p ignite -p ignite-rt >>$LOG 2>&1
rc1=$?
echo "BUILD rc=$rc1" >>$LOG
cargo clippy -p ignite --all-targets -- -D warnings >>$LOG 2>&1
rc2=$?
echo "CLIPPY rc=$rc2" >>$LOG
cargo fmt --all -- --check >>$LOG 2>&1
rc3=$?
echo "FMT rc=$rc3" >>$LOG
echo "EXIT=$((rc1 + rc2 + rc3)) $(date -Is)" >>$LOG
