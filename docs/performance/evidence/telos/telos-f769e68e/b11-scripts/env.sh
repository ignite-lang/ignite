S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
LOGS=$S/b11-logs
BUILDS=$S/b11-build
mkdir -p $LOGS $BUILDS
export CARGO_TARGET_DIR=$S/target-11b
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
IGNITE=$CARGO_TARGET_DIR/debug/ignite
RT=$CARGO_TARGET_DIR/ignite-test/rt/debug/libignite_rt.a
REPO=/home/timo/projects/telos
