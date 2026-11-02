#!/bin/zsh
# usage: accept.sh : 15.0 items 4, 5, 6 and 11 on the working tree with the P4 ignite binary.
S=/home/timo/.cache/scratch/f769e68e-6990-4c7f-be9b-561eb71b2bb6
export CARGO_TARGET_DIR=$S/target-p4
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
ulimit -c 0
REPO=/home/timo/projects/telos
IGNITE=$S/target-p4/debug/ignite
DESIGN=$REPO/examples/terminal/terminal.design.ignite
GOLD=$REPO/crates/ignite/tests/golden/terminal.design.model.json
OUT=$S/p4-build/accept
mkdir -p $OUT/wd1 $OUT/wd2
echo "== item 4: two plan runs from two working directories, byte-identical and equal to the golden"
( cd $OUT/wd1 && $IGNITE plan $DESIGN > $OUT/plan1.json; echo "wd1 abs exit=$?" )
( cd $REPO && $IGNITE plan examples/terminal/terminal.design.ignite > $OUT/plan2.json; echo "repo rel exit=$?" )
( cd $OUT/wd2 && $IGNITE plan $DESIGN --out $OUT/plan3.json; echo "wd2 --out exit=$?" )
cmp $OUT/plan1.json $OUT/plan2.json && echo "plan1 == plan2"
cmp $OUT/plan1.json $OUT/plan3.json && echo "plan1 == plan3"
cmp $OUT/plan1.json $GOLD && echo "plan1 == golden"
sha256sum $OUT/plan1.json $OUT/plan2.json $OUT/plan3.json $GOLD
echo "== item 5: no expression text, no quantity with a unit"
echo "calls: $(grep -c -E 'exponential\(|rolling\(|calendar_month\(|event\(|module\(|localSocket\(|nativeProcess\(|workstationExecutable\(|scheduledProbeHost\(|endpoint\(' $OUT/plan1.json)"
echo "units: $(grep -c -E '[0-9] ?(ns|us|ms|seconds?|minutes?|hours?|days?|bytes?|KiB|MiB|GiB)[\" ,]|[0-9](\.[0-9]+)?%' $OUT/plan1.json)"
python3 -I $S/p4-scripts/accept_model.py $OUT/plan1.json
echo "== item 6: ignite check of the design file"
$IGNITE check $DESIGN; echo "check exit=$?"
echo "== item 11: fixtures E110 to E119"
for code in e110 e111 e112 e113 e114 e115 e116 e117 e118 e119; do
  found=$(find $REPO/crates/ignite/tests/fixtures $REPO/crates/ignite-types/tests/fixtures $REPO/crates/ignite-eval/tests -name "${code}_*" 2>/dev/null | sed "s#$REPO/##" | sort | tr '\n' ' ')
  echo "$code: ${found:-MISSING}"
done
echo "EXIT=0"
