#!/bin/zsh
# P2 slice 12 chain 14: todod pairs after unit 5 again (the chain-13 pairs ran at load 11 to 27),
# then the cold split of the new tree's ignite-io / ignite-std / program builds.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_RELEASE_DEBUG=0
ulimit -c 0
load1() { cut -d' ' -f1 /proc/loadavg }
secs() { local t0=$(date +%s.%N); "$@" > /dev/null 2>&1; local rc=$?; local t1=$(date +%s.%N); printf '%.3f' $(echo "$t1 - $t0" | bc); return $rc }
cd /home/timo/projects/telos || exit 9
echo "== todod pairs after unit 5, second set"
zsh $S/s12-p2-todod.sh unit5cold2 3
echo "STEP todod exit=$?"
echo "== split, rust, cold (SCCACHE_RECACHE=1): base vs new"
export SCCACHE_RECACHE=1
for i in 1 2; do
  for which in base new; do
    for profile in dev release; do
      p=$S/s12-p2-bench-unit5cold2/$which-rust-$profile-1/project
      t=$S/s12-p2-split5-$which-$profile-$i
      rm -rf $t
      flag=(); [ $profile = release ] && flag=(--release)
      io=$(secs cargo build --manifest-path $p/Cargo.toml --target-dir $t $flag -p ignite-io)
      std=$(secs cargo build --manifest-path $p/Cargo.toml --target-dir $t $flag -p ignite-std)
      prog=$(secs cargo build --manifest-path $p/Cargo.toml --target-dir $t $flag)
      echo "SPLIT rust $which $profile run=$i io=$io std=$std program=$prog load=$(load1)"
      rm -rf $t
    done
  done
done
echo "EXIT=0"
